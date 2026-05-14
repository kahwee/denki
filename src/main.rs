mod resolve;

use denki::cli::{Cli, Command, LedAction};
use denki::devices::{self, DeviceKind};
use denki::{bulb, creds, dimmer, display, fmt, hosts, klap, ops, plug, strip, tapo, transport};
use resolve::{require_kasa, resolve, resolve_outlet, resolve_quiet};

use anyhow::{bail, Result};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use colored::Colorize;

async fn tapo_session(ip: &str) -> Result<klap::KlapSession> {
    let (user, pass) = creds::load()?;
    klap::handshake(ip, &user, &pass).await
}

/// Returns the target on/off state when toggling: true = turn on, false = turn off.
/// Strip: relay_state is absent on HS300 HW 2.0 — derive from child outlet states instead.
fn toggle_target(kind: &DeviceKind, json: &serde_json::Value) -> bool {
    match kind {
        DeviceKind::Bulb => {
            json.pointer("/system/get_sysinfo/light_state/on_off")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                == 0
        }
        DeviceKind::Strip => !strip::parse(json).map(|s| s.is_any_on()).unwrap_or(false),
        _ => {
            json.pointer("/system/get_sysinfo/relay_state")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                == 0
        }
    }
}

// Execute on or off on a Kasa device.
async fn kasa_set_power(ip: &str, kind: &DeviceKind, on: bool) -> Result<()> {
    devices::can_control_power(kind)?;
    match (kind, on) {
        (DeviceKind::Bulb, true) => ops::bulb_on(ip).await?,
        (DeviceKind::Bulb, false) => ops::bulb_off(ip).await?,
        (_, true) => ops::relay_on(ip).await?,
        (_, false) => ops::relay_off(ip).await?,
    }
    Ok(())
}

// Resolve an outlet on a strip: require Kasa, fetch sysinfo, parse strip, return
// (child_id, child_alias, child_is_on). Used by on/off/toggle outlet paths.
async fn resolve_strip_outlet(
    r: &resolve::Resolved,
    cmd: &str,
    outlet: u8,
) -> Result<(String, String, bool)> {
    require_kasa(r, cmd)?;
    let json = ops::sysinfo(&r.ip).await?;
    let s = strip::parse(&json)
        .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
    let child = resolve_outlet(&s, outlet)?;
    Ok((child.id.clone(), child.alias.clone(), child.is_on()))
}

// Returns the resolved strip, child_id, and child_alias for per-outlet energy commands.
// Fails clearly if the JSON is not a strip, the strip has no ENE chip, or the outlet is out of range.
fn strip_for_energy_outlet(
    json: &serde_json::Value,
    ip: &str,
    outlet: u8,
) -> Result<(String, String)> {
    let s = strip::parse(json)
        .ok_or_else(|| anyhow::anyhow!("{ip} does not appear to be a power strip"))?;
    if !s.has_energy_monitoring() {
        anyhow::bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
    }
    let child = resolve_outlet(&s, outlet)?;
    Ok((child.id.clone(), child.alias.clone()))
}

/// Parse "YYYY-MM" and validate month is 1–12.
fn parse_year_month(s: &str) -> Result<(u16, u8)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        bail!("Month must be in YYYY-MM format");
    }
    let year: u16 = parts[0].parse()?;
    let mo: u8 = parts[1].parse()?;
    if !(1..=12).contains(&mo) {
        bail!("Month must be 01–12, got {mo:02}");
    }
    Ok((year, mo))
}

/// Resolve a Kasa host, require Kasa protocol, fetch sysinfo, and detect device kind.
async fn kasa_sysinfo(
    host: &str,
    cmd: &str,
) -> Result<(resolve::Resolved, serde_json::Value, DeviceKind)> {
    let r = resolve(host).await?;
    require_kasa(&r, cmd)?;
    let json = ops::sysinfo(&r.ip).await?;
    let kind = devices::detect_kind(&json);
    Ok((r, json, kind))
}

/// Dispatch real-time energy to the correct namespace (bulb vs relay device).
async fn energy_realtime_for(ip: &str, kind: &DeviceKind) -> Result<serde_json::Value> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy(ip).await,
        _ => ops::device_energy(ip).await,
    }
}

/// Dispatch daily energy to the correct namespace.
async fn energy_daily_for(
    ip: &str,
    kind: &DeviceKind,
    year: u16,
    mo: u8,
) -> Result<serde_json::Value> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy_daily(ip, year, mo).await,
        _ => ops::device_energy_daily(ip, year, mo).await,
    }
}

/// Dispatch monthly energy to the correct namespace.
async fn energy_monthly_for(
    ip: &str,
    kind: &DeviceKind,
    year: u16,
) -> Result<serde_json::Value> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy_monthly(ip, year).await,
        _ => ops::device_energy_monthly(ip, year).await,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { timeout } => {
            println!("{}", format!("Scanning network for {timeout}s...").dimmed());
            let mut host_map = hosts::load().unwrap_or_default();
            let mut map_dirty = false;
            let mut device_count = transport::broadcast_each(timeout, |ip, json| {
                let ip_str = ip.to_string();
                let is_new = json
                    .pointer("/system/get_sysinfo/alias")
                    .and_then(|v| v.as_str())
                    .map(|name| hosts::save_if_new_in(name, &ip_str, &mut host_map))
                    .unwrap_or(false);
                if is_new {
                    map_dirty = true;
                }
                let hint = hosts::lookup_by_ip_in(&ip_str, &host_map).unwrap_or_else(|| ip_str.clone());
                match devices::detect_kind(&json) {
                    DeviceKind::Bulb => {
                        if let Some(b) = bulb::parse(&json) {
                            display::print_bulb_summary(ip, &b, &hint);
                        }
                    }
                    DeviceKind::LightStrip => {
                        if let Some(b) = bulb::parse(&json) {
                            display::print_lightstrip_summary(ip, &b, &hint);
                        }
                    }
                    DeviceKind::Dimmer => {
                        if let Some(d) = dimmer::parse(&json) {
                            display::print_dimmer_summary(ip, &d, &hint);
                        }
                    }
                    DeviceKind::Strip => {
                        if let Some(s) = strip::parse(&json) {
                            display::print_strip_summary(ip, &s, &hint);
                        }
                    }
                    DeviceKind::Plug => {
                        if let Some(p) = plug::parse(&json) {
                            display::print_plug_summary(ip, &p, &hint);
                        }
                    }
                    // Tapo devices use KLAP on port 80 and won't respond to UDP probe.
                    DeviceKind::Tapo => display::print_unknown_summary(ip, &json, "tapo"),
                    DeviceKind::Unknown(t) => display::print_unknown_summary(ip, &json, &t),
                }
                if is_new {
                    println!("{}", "  ↳ (new) alias auto-saved".dimmed());
                }
            })
            .await?;
            if map_dirty {
                hosts::save(&host_map)?;
            }

            let klap_aliases: Vec<(String, hosts::HostEntry)> = host_map
                .into_iter()
                .filter(|(_, v)| v.protocol == hosts::Protocol::Klap)
                .collect();
            let mut join_set = tokio::task::JoinSet::new();
            for (name, entry) in klap_aliases {
                join_set.spawn(async move {
                    let ip = entry.ip;
                    let mut session = tapo_session(&ip).await.ok()?;
                    let json = ops::tapo_device_info(&mut session).await.ok()?;
                    let d = tapo::parse(&json)?;
                    Some((ip, name, d))
                });
            }
            while let Some(result) = join_set.join_next().await {
                if let Ok(Some((ip, name, d))) = result {
                    display::print_tapo_summary(&ip, &d, &name);
                    device_count += 1;
                }
            }

            if device_count == 0 {
                println!("No devices found.");
            } else {
                println!("{}", format!("Found {device_count} device(s)").dimmed());
            }
        }

        Command::Info { host } => {
            let r = resolve_quiet(&host).await?;
            let hint = r.saved_name.as_deref().unwrap_or(&r.ip).to_string();
            match r.protocol {
                hosts::Protocol::Klap => {
                    let mut session = tapo_session(&r.ip).await?;
                    let json = ops::tapo_device_info(&mut session).await?;
                    match tapo::parse(&json) {
                        Some(d) => display::print_tapo_detail(&r.ip, &d, &hint),
                        None => bail!("Could not parse Tapo device info from {}", r.ip),
                    }
                }
                hosts::Protocol::Kasa => {
                    let json = ops::sysinfo(&r.ip).await?;
                    let kind = devices::detect_kind(&json);
                    match kind {
                        DeviceKind::Bulb => match bulb::parse(&json) {
                            Some(b) => display::print_bulb_detail(&r.ip, &b, &hint),
                            None => bail!("Could not parse bulb sysinfo from {}", r.ip),
                        },
                        DeviceKind::LightStrip => match bulb::parse(&json) {
                            Some(b) => display::print_lightstrip_detail(&r.ip, &b),
                            None => bail!("Could not parse light strip sysinfo from {}", r.ip),
                        },
                        DeviceKind::Dimmer => match dimmer::parse(&json) {
                            Some(d) => display::print_dimmer_detail(&r.ip, &d, &hint),
                            None => bail!("Could not parse dimmer sysinfo from {}", r.ip),
                        },
                        DeviceKind::Strip => match strip::parse(&json) {
                            Some(s) => display::print_strip_detail(&r.ip, &s, &hint),
                            None => bail!("Could not parse strip sysinfo from {}", r.ip),
                        },
                        DeviceKind::Plug => match plug::parse(&json) {
                            Some(p) => display::print_plug_detail(&r.ip, &p, &hint),
                            None => bail!("Could not parse plug sysinfo from {}", r.ip),
                        },
                        DeviceKind::Tapo | DeviceKind::Unknown(_) => {
                            eprintln!("{}", format!("Unsupported device type: {kind}").yellow());
                            eprintln!("Raw sysinfo from {}:", r.ip);
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json)
                                    .unwrap_or_else(|_| json.to_string())
                            );
                        }
                    }
                }
            }
        }

        Command::On { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                let (child_id, child_alias, _) =
                    resolve_strip_outlet(&r, "on <outlet>", outlet_num).await?;
                ops::strip_outlet_on(&r.ip, &child_id).await?;
                println!("Outlet {} ({}) {}", outlet_num, child_alias, "on".green().bold());
            } else {
                match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = tapo_session(&r.ip).await?;
                        ops::tapo_on(&mut s).await?;
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_set_power(&r.ip, &devices::detect_kind(&json), true).await?;
                    }
                }
                println!("{} {}", r.ip, "on".green().bold());
            }
        }

        Command::Off { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                let (child_id, child_alias, _) =
                    resolve_strip_outlet(&r, "off <outlet>", outlet_num).await?;
                ops::strip_outlet_off(&r.ip, &child_id).await?;
                println!("Outlet {} ({}) {}", outlet_num, child_alias, "off".dimmed());
            } else {
                match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = tapo_session(&r.ip).await?;
                        ops::tapo_off(&mut s).await?;
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_set_power(&r.ip, &devices::detect_kind(&json), false).await?;
                    }
                }
                println!("{} {}", r.ip, "off".dimmed());
            }
        }

        Command::Toggle { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                let (child_id, child_alias, was_on) =
                    resolve_strip_outlet(&r, "toggle <outlet>", outlet_num).await?;
                let now_on = if was_on {
                    ops::strip_outlet_off(&r.ip, &child_id).await?;
                    false
                } else {
                    ops::strip_outlet_on(&r.ip, &child_id).await?;
                    true
                };
                let label = if now_on { "on".green().bold() } else { "off".dimmed() };
                println!("Outlet {} ({}) -> {label}", outlet_num, child_alias);
            } else {
                let now_on = match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = tapo_session(&r.ip).await?;
                        ops::tapo_toggle(&mut s).await?
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        let kind = devices::detect_kind(&json);
                        let on = toggle_target(&kind, &json);
                        kasa_set_power(&r.ip, &kind, on).await?;
                        on
                    }
                };
                let label = if now_on { "on".green().bold() } else { "off".dimmed() };
                println!("{} -> {label}", r.ip);
            }
        }

        Command::Dim { host, level } => {
            let (r, json, kind) = kasa_sysinfo(&host, "dim").await?;
            devices::can_dim(&kind)?;
            match kind {
                DeviceKind::Bulb => {
                    // Turn on first if currently off (setting brightness while off has no visible effect)
                    if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                        ops::bulb_on(&r.ip).await?;
                    }
                    ops::bulb_set_brightness(&r.ip, level).await?;
                }
                DeviceKind::Dimmer => {
                    if level > 0 && dimmer::parse(&json).is_some_and(|d| !d.is_on()) {
                        ops::relay_on(&r.ip).await?;
                    }
                    ops::dimmer_set_brightness(&r.ip, level).await?;
                }
                _ => unreachable!(),
            }
            println!("Brightness -> {level}%");
        }

        Command::ColorTemp { host, kelvin } => {
            let (r, json, kind) = kasa_sysinfo(&host, "color-temp").await?;
            devices::can_set_color_temp(&kind)?;
            if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                ops::bulb_on(&r.ip).await?;
            }
            ops::bulb_set_color_temp(&r.ip, kelvin).await?;
            println!("Color temperature -> {kelvin}K");
        }

        Command::Color { host, hue, saturation, value } => {
            let (r, json, kind) = kasa_sysinfo(&host, "color").await?;
            devices::can_set_color(&kind)?;
            if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                ops::bulb_on(&r.ip).await?;
            }
            ops::bulb_set_color(&r.ip, hue, saturation, value).await?;
            println!("Color -> hue:{hue} sat:{saturation} val:{value}");
        }

        Command::Energy { host, outlet } => {
            let (r, json, kind) = kasa_sysinfo(&host, "energy").await?;
            if let Some(outlet_num) = outlet {
                let (child_id, child_alias) =
                    strip_for_energy_outlet(&json, &r.ip, outlet_num)?;
                let resp = ops::strip_outlet_energy(&r.ip, &child_id).await?;
                println!("Outlet {} ({})", outlet_num, child_alias.bold());
                display::print_energy_realtime(&resp);
            } else {
                devices::require_energy(&json, &kind)?;
                display::print_energy_realtime(&energy_realtime_for(&r.ip, &kind).await?);
            }
        }

        Command::EnergyDaily { host, month, outlet } => {
            let (r, json, kind) = kasa_sysinfo(&host, "energy-daily").await?;
            let month_str = month.unwrap_or_else(|| {
                let (y, m) = fmt::current_year_month();
                format!("{y}-{m:02}")
            });
            let (year, mo) = parse_year_month(&month_str)?;
            if let Some(outlet_num) = outlet {
                let (child_id, child_alias) =
                    strip_for_energy_outlet(&json, &r.ip, outlet_num)?;
                let resp = ops::strip_outlet_energy_daily(&r.ip, &child_id, year, mo).await?;
                println!("Outlet {} ({})", outlet_num, child_alias.bold());
                display::print_energy_daily(&resp, &month_str);
            } else {
                devices::require_energy(&json, &kind)?;
                display::print_energy_daily(
                    &energy_daily_for(&r.ip, &kind, year, mo).await?,
                    &month_str,
                );
            }
        }

        Command::EnergyMonthly { host, year, outlet } => {
            let (r, json, kind) = kasa_sysinfo(&host, "energy-monthly").await?;
            let year = year.unwrap_or_else(|| fmt::current_year_month().0);
            if let Some(outlet_num) = outlet {
                let (child_id, child_alias) =
                    strip_for_energy_outlet(&json, &r.ip, outlet_num)?;
                let resp = ops::strip_outlet_energy_monthly(&r.ip, &child_id, year).await?;
                println!("Outlet {} ({})", outlet_num, child_alias.bold());
                display::print_energy_monthly(&resp, year);
            } else {
                devices::require_energy(&json, &kind)?;
                display::print_energy_monthly(&energy_monthly_for(&r.ip, &kind, year).await?, year);
            }
        }

        Command::Specs { host } => {
            let (r, _, kind) = kasa_sysinfo(&host, "specs").await?;
            devices::can_get_specs(&kind)?;
            display::print_bulb_specs(&ops::bulb_specs(&r.ip).await?);
        }

        Command::Presets { host } => {
            let (r, _, kind) = kasa_sysinfo(&host, "presets").await?;
            devices::can_get_presets(&kind)?;
            display::print_bulb_presets(&ops::bulb_presets(&r.ip).await?);
        }

        Command::Schedules { host } => {
            let (r, _, kind) = kasa_sysinfo(&host, "schedules").await?;
            devices::can_get_schedules(&kind)?;
            display::print_schedules(&ops::device_schedules(&r.ip).await?);
        }

        Command::Led { host, state } => {
            let (r, _, kind) = kasa_sysinfo(&host, "led").await?;
            devices::can_control_led(&kind)?;
            let on = matches!(state, LedAction::On);
            ops::device_led(&r.ip, on).await?;
            println!("LED indicator {}", if on { "on".green() } else { "off".dimmed() });
        }

        Command::Clock { host } => {
            let (r, _, kind) = kasa_sysinfo(&host, "clock").await?;
            devices::can_get_clock(&kind)?;
            let resp = ops::device_time(&r.ip).await?;
            if let Some(t) = resp.pointer("/time/get_time") {
                println!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    t["year"].as_u64().unwrap_or(0),
                    t["month"].as_u64().unwrap_or(0),
                    t["mday"].as_u64().unwrap_or(0),
                    t["hour"].as_u64().unwrap_or(0),
                    t["min"].as_u64().unwrap_or(0),
                    t["sec"].as_u64().unwrap_or(0),
                );
            } else {
                bail!("Unexpected response from {}: no time data", r.ip);
            }
        }

        Command::Rename { host, name } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "rename")?;
            ops::rename(&r.ip, &name).await?;
            println!("Renamed to \"{}\"", name.bold());
        }

        Command::Restart { host } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "restart")?;
            ops::restart(&r.ip).await?;
            println!("{} rebooting...", r.ip);
        }

        Command::Outlets { host } => {
            let (r, json, _) = kasa_sysinfo(&host, "outlets").await?;
            match strip::parse(&json) {
                Some(s) => display::print_strip_outlets(&s),
                None => bail!("{} does not appear to be a power strip", r.ip),
            }
        }

        Command::OutletRename { host, outlet, name } => {
            let (r, json, _) = kasa_sysinfo(&host, "outlet-rename").await?;
            let s = strip::parse(&json)
                .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
            let child = resolve_outlet(&s, outlet)?;
            let old_name = child.alias.clone();
            ops::strip_outlet_rename(&r.ip, &child.id, &name).await?;
            println!("Outlet {} renamed: {} → {}", outlet, old_name, name.bold());
        }

        Command::Alias { name, ip, klap } => {
            let protocol = if klap { hosts::Protocol::Klap } else { hosts::Protocol::Kasa };
            hosts::set(&name, &ip, protocol)?;
            let tag = if klap { " (klap)".dimmed() } else { "".normal() };
            println!("Saved: {} → {}{}", name.bold(), ip, tag);
        }

        Command::Unalias { name } => {
            if hosts::remove(&name)? {
                println!("Removed alias \"{}\"", name);
            } else {
                bail!("No alias named \"{name}\" found");
            }
        }

        Command::Aliases => {
            let list = hosts::list()?;
            if list.is_empty() {
                println!("No saved aliases. Use `denki alias <name> <ip> [--klap]` to add one.");
                println!("File: {}", hosts::path_display());
            } else {
                println!(
                    "{:<30} {:<18} {}",
                    "Name".bold(),
                    "IP".bold(),
                    "Protocol".bold()
                );
                println!("{}", "─".repeat(58).dimmed());
                for (name, entry) in &list {
                    println!("{:<30} {:<18} {}", name, entry.ip, entry.protocol);
                }
                println!(
                    "{}",
                    format!("({} aliases in {})", list.len(), hosts::path_display()).dimmed()
                );
            }
        }

        Command::Login { email, password } => {
            let password = match password {
                Some(p) => p,
                None => rpassword::prompt_password("Tapo password: ")
                    .map_err(|e| anyhow::anyhow!("Failed to read password: {e}"))?,
            };
            creds::save(&email, &password)?;
            println!("Tapo credentials saved to {}", creds::path_display());
            println!(
                "(File is readable only by you. Use TAPO_USER/TAPO_PASS env vars to override.)"
            );
        }

        Command::Completions { shell } => {
            generate(shell, &mut Cli::command(), "denki", &mut std::io::stdout());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use denki::cli::{Cli, Command};
    use serde_json::json;

    #[test]
    fn toggle_target_bulb_on_returns_false() {
        let json = json!({"system": {"get_sysinfo": {"light_state": {"on_off": 1}}}});
        assert!(!toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_bulb_off_returns_true() {
        let json = json!({"system": {"get_sysinfo": {"light_state": {"on_off": 0}}}});
        assert!(toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_plug_on_returns_false() {
        let json = json!({"system": {"get_sysinfo": {"relay_state": 1}}});
        assert!(!toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_plug_off_returns_true() {
        let json = json!({"system": {"get_sysinfo": {"relay_state": 0}}});
        assert!(toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_strip_any_on_returns_false() {
        let json = json!({
            "system": {"get_sysinfo": {
                "alias": "Strip", "model": "HS300(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -40, "feature": "TIM",
                "children": [{"id": "A1", "alias": "Outlet 1", "state": 1},
                             {"id": "A2", "alias": "Outlet 2", "state": 0}]
            }}
        });
        assert!(!toggle_target(&DeviceKind::Strip, &json));
    }

    #[test]
    fn toggle_target_strip_all_off_returns_true() {
        let json = json!({
            "system": {"get_sysinfo": {
                "alias": "Strip", "model": "HS300(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -40, "feature": "TIM",
                "children": [{"id": "A1", "alias": "Outlet 1", "state": 0},
                             {"id": "A2", "alias": "Outlet 2", "state": 0}]
            }}
        });
        assert!(toggle_target(&DeviceKind::Strip, &json));
    }

    #[test]
    fn strip_toggle_uses_child_states_not_relay_state() {
        // HS300 HW 2.0 omits relay_state; is_any_on() must be used instead.
        let json = json!({
            "system": { "get_sysinfo": {
                "alias": "Test Strip", "model": "HS300(US)",
                "hw_ver": "2.0", "sw_ver": "1.0.0", "rssi": -50,
                "mic_type": "IOT.SMARTPLUGSWITCH",
                "feature": "TIM:ENE",
                "children": [
                    { "id": "8006C2", "alias": "Outlet 1", "state": 1 },
                    { "id": "8006C3", "alias": "Outlet 2", "state": 0 }
                ]
            }}
        });
        let s = strip::parse(&json).expect("should parse");
        assert_eq!(s.relay_state, 0, "relay_state absent → deserialized as 0");
        assert!(s.is_any_on(), "is_any_on should be true when any child state == 1");
    }

    #[test]
    fn alias_matching_is_case_and_punctuation_insensitive() {
        fn matches(alias: &str, query: &str) -> bool {
            let a = hosts::normalize(alias);
            let q = hosts::normalize(query);
            !q.is_empty() && (a == q || a.contains(&q))
        }
        assert!(matches("Living Room Right Lamp", "living room"));
        assert!(matches("Coat-Rack Lights", "coat rack"));
        assert!(matches("Kitchen Wax Melter", "KITCHEN"));
        assert!(!matches("Back Porch Reading Lamp", "coat rack"));
    }

    #[test]
    fn on_off_toggle_parse_as_top_level_commands() {
        let on = Cli::try_parse_from(["denki", "on", "desk lamp"]).unwrap();
        assert!(matches!(on.command, Command::On { ref host, .. } if host == "desk lamp"));

        let off = Cli::try_parse_from(["denki", "off", "desk lamp"]).unwrap();
        assert!(matches!(off.command, Command::Off { ref host, .. } if host == "desk lamp"));

        let tog = Cli::try_parse_from(["denki", "toggle", "desk lamp"]).unwrap();
        assert!(matches!(tog.command, Command::Toggle { ref host, .. } if host == "desk lamp"));
    }

    #[test]
    fn power_subcommand_no_longer_exists() {
        assert!(Cli::try_parse_from(["denki", "power", "desk lamp", "on"]).is_err());
    }

    #[test]
    fn dim_command_parses_host_and_level() {
        let cli = Cli::try_parse_from(["denki", "dim", "desk lamp", "75"]).unwrap();
        assert!(
            matches!(cli.command, Command::Dim { host, level } if host == "desk lamp" && level == 75)
        );
    }

    #[test]
    fn dim_rejects_level_above_100() {
        assert!(Cli::try_parse_from(["denki", "dim", "desk lamp", "101"]).is_err());
    }

    #[test]
    fn on_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "on", "strip", "2"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::On { ref host, outlet: Some(2) } if host == "strip"
        ));
    }

    #[test]
    fn on_without_outlet_parses_host_only() {
        let cli = Cli::try_parse_from(["denki", "on", "strip"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::On { ref host, outlet: None } if host == "strip"
        ));
    }

    #[test]
    fn on_rejects_zero_outlet() {
        assert!(Cli::try_parse_from(["denki", "on", "strip", "0"]).is_err());
    }

    #[test]
    fn off_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "off", "strip", "3"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Off { ref host, outlet: Some(3) } if host == "strip"
        ));
    }

    #[test]
    fn off_rejects_zero_outlet() {
        assert!(Cli::try_parse_from(["denki", "off", "strip", "0"]).is_err());
    }

    #[test]
    fn toggle_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "toggle", "strip", "2"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Toggle { ref host, outlet: Some(2) } if host == "strip"
        ));
    }

    #[test]
    fn energy_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "energy", "strip", "1"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Energy { ref host, outlet: Some(1) } if host == "strip"
        ));
    }

    #[test]
    fn energy_without_outlet_parses_host_only() {
        let cli = Cli::try_parse_from(["denki", "energy", "strip"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Energy { ref host, outlet: None } if host == "strip"
        ));
    }

    #[test]
    fn energy_daily_with_outlet_flag() {
        let cli = Cli::try_parse_from(["denki", "energy-daily", "strip", "--outlet", "2"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::EnergyDaily { ref host, outlet: Some(2), .. } if host == "strip"
        ));
    }

    #[test]
    fn energy_monthly_with_outlet_flag() {
        let cli = Cli::try_parse_from(["denki", "energy-monthly", "strip", "--outlet", "1"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::EnergyMonthly { ref host, outlet: Some(1), .. } if host == "strip"
        ));
    }

    #[test]
    fn parse_year_month_valid() {
        assert_eq!(parse_year_month("2025-03").unwrap(), (2025, 3));
        assert_eq!(parse_year_month("2024-12").unwrap(), (2024, 12));
        assert_eq!(parse_year_month("2025-01").unwrap(), (2025, 1));
    }

    #[test]
    fn parse_year_month_rejects_month_zero() {
        assert!(parse_year_month("2025-00").is_err());
    }

    #[test]
    fn parse_year_month_rejects_month_13() {
        assert!(parse_year_month("2025-13").is_err());
    }

    #[test]
    fn parse_year_month_rejects_wrong_format() {
        assert!(parse_year_month("202503").is_err());
        assert!(parse_year_month("2025-03-01").is_err());
    }

    // ── strip_for_energy_outlet ───────────────────────────────────────────────

    fn ene_strip_json(n: u8) -> serde_json::Value {
        let children: Vec<serde_json::Value> = (0..n)
            .map(|i| json!({"id": format!("ID{i:02}"), "state": 0, "alias": format!("Outlet {}", i + 1)}))
            .collect();
        json!({
            "system": { "get_sysinfo": {
                "alias": "Test Strip", "model": "HS300(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -40,
                "feature": "TIM:ENE", "children": children
            }}
        })
    }

    fn no_ene_strip_json() -> serde_json::Value {
        json!({
            "system": { "get_sysinfo": {
                "alias": "Basic Strip", "model": "KP303(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -50,
                "feature": "TIM",
                "children": [{"id": "A1", "state": 0, "alias": "Outlet 1"}]
            }}
        })
    }

    #[test]
    fn strip_for_energy_outlet_succeeds_on_ene_strip() {
        let json = ene_strip_json(3);
        let (id, alias) = strip_for_energy_outlet(&json, "1.2.3.4", 2).unwrap();
        assert_eq!(id, "ID01");
        assert_eq!(alias, "Outlet 2");
    }

    #[test]
    fn strip_for_energy_outlet_fails_on_non_strip_json() {
        let err = strip_for_energy_outlet(&json!({}), "1.2.3.4", 1).unwrap_err();
        assert!(err.to_string().contains("power strip"), "{err}");
    }

    #[test]
    fn strip_for_energy_outlet_fails_without_ene() {
        let json = no_ene_strip_json();
        let err = strip_for_energy_outlet(&json, "1.2.3.4", 1).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("energy monitoring"), "{msg}");
        assert!(msg.contains("KP303"), "{msg}");
    }

    #[test]
    fn strip_for_energy_outlet_fails_on_out_of_range_outlet() {
        let json = ene_strip_json(2);
        let err = strip_for_energy_outlet(&json, "1.2.3.4", 5).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("outlet 5"), "{msg}");
        assert!(msg.contains("2 outlets"), "{msg}");
    }

    #[test]
    fn strip_for_energy_outlet_outlet_1_succeeds() {
        let json = ene_strip_json(1);
        let (id, alias) = strip_for_energy_outlet(&json, "1.2.3.4", 1).unwrap();
        assert_eq!(id, "ID00");
        assert_eq!(alias, "Outlet 1");
    }

    // ── toggle_target (kasa_set_power logic covered by integration) ───────────

    #[test]
    fn toggle_target_missing_fields_defaults_to_on() {
        // Missing relay_state → unwrap_or(0) → should return true (turn on)
        let json = json!({"system": {"get_sysinfo": {}}});
        assert!(toggle_target(&DeviceKind::Plug, &json));
    }
}

// Every (model, feature) in devices.toml must pass the corresponding guard,
// and every guarded feature NOT listed must be denied.
#[cfg(test)]
mod capability_tests {
    use denki::devices::{self, DeviceKind};

    fn guard_kind(kind: &DeviceKind) -> Option<&DeviceKind> {
        match kind {
            DeviceKind::Tapo => None,
            k => Some(k),
        }
    }

    const GUARDED: &[&str] = &[
        "power", "dim", "color_temp", "color", "specs", "presets", "schedules", "led", "clock",
    ];

    fn check(kind: &DeviceKind, feature: &str) -> anyhow::Result<()> {
        match feature {
            "power" => devices::can_control_power(kind),
            "dim" => devices::can_dim(kind),
            "color_temp" => devices::can_set_color_temp(kind),
            "color" => devices::can_set_color(kind),
            "specs" => devices::can_get_specs(kind),
            "presets" => devices::can_get_presets(kind),
            "schedules" => devices::can_get_schedules(kind),
            "led" => devices::can_control_led(kind),
            "clock" => devices::can_get_clock(kind),
            "energy" | "outlets" => Ok(()), // runtime-checked, no static guard
            other => panic!(
                "devices.toml: unknown feature '{other}' — add it to check() or explain \
                 why it has no guard"
            ),
        }
    }

    #[test]
    fn listed_features_are_permitted_by_guards() {
        for dev in devices::all() {
            let Some(kind) = guard_kind(&dev.kind) else { continue };
            for feature in &dev.supports {
                let result = check(kind, feature);
                assert!(
                    result.is_ok(),
                    "devices.toml: {} ({}) lists '{}' but the guard rejects it: {}",
                    dev.model,
                    dev.kind,
                    feature,
                    result.unwrap_err(),
                );
            }
        }
    }

    #[test]
    fn unlisted_guarded_features_are_denied() {
        for dev in devices::all() {
            let Some(kind) = guard_kind(&dev.kind) else { continue };
            for &feature in GUARDED {
                if dev.supports.iter().any(|f| f == feature) {
                    continue;
                }
                let result = check(kind, feature);
                assert!(
                    result.is_err(),
                    "devices.toml: {} ({}) does NOT list '{}' but the guard permits it — \
                     add it to 'supports' or tighten the guard",
                    dev.model,
                    dev.kind,
                    feature,
                );
            }
        }
    }
}
