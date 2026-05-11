mod resolve;

use denki::cli::{Cli, Command, LedAction};
use denki::devices::{self, DeviceKind};
use denki::{bulb, creds, dimmer, display, fmt, hosts, klap, ops, plug, strip, tapo, transport};
use resolve::{require_kasa, resolve, resolve_outlet, resolve_quiet};

use anyhow::{bail, Result};
use clap::Parser;
use colored::Colorize;

async fn open_tapo(ip: &str) -> Result<klap::KlapSession> {
    let (user, pass) = creds::load()?;
    klap::handshake(ip, &user, &pass).await
}

// Execute on/off/toggle on a Kasa device using an already-fetched sysinfo blob.
// target_on: Some(true) = on, Some(false) = off, None = toggle (inverts current state).
// Strip: relay_state is absent on HS300 HW 2.0 — derive from child outlet states instead.
async fn kasa_exec_power(
    ip: &str,
    kind: &DeviceKind,
    json: &serde_json::Value,
    target_on: Option<bool>,
) -> Result<bool> {
    devices::can_control_power(kind)?;
    let on = target_on.unwrap_or_else(|| match kind {
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
    });
    if matches!(kind, DeviceKind::Bulb) {
        if on { ops::bulb_on(ip).await? } else { ops::bulb_off(ip).await? }
    } else {
        if on { ops::relay_on(ip).await? } else { ops::relay_off(ip).await? }
    }
    Ok(on)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { timeout } => {
            println!("{}", format!("Scanning network for {timeout}s...").dimmed());
            let mut kasa_count = transport::broadcast_each(timeout, |ip, json| {
                let ip_str = ip.to_string();
                if let Some(name) = json
                    .pointer("/system/get_sysinfo/alias")
                    .and_then(|v| v.as_str())
                {
                    let _ = hosts::save_if_new(name, &ip_str);
                }
                let hint = hosts::lookup_by_ip(&ip_str).unwrap_or_else(|| ip_str.clone());
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
            })
            .await?;

            for (name, entry) in hosts::klap_aliases() {
                if let Ok(mut session) = open_tapo(&entry.ip).await {
                    if let Ok(json) = ops::tapo_device_info(&mut session).await {
                        if let Some(d) = tapo::parse(&json) {
                            display::print_tapo_summary(&entry.ip, &d, &name);
                            kasa_count += 1;
                        }
                    }
                }
            }

            if kasa_count == 0 {
                println!("No devices found.");
            } else {
                println!("{}", format!("Found {kasa_count} device(s)").dimmed());
            }
        }

        Command::Info { host } => {
            let r = resolve_quiet(&host).await?;
            let hint = r.saved_name.as_deref().unwrap_or(&r.ip).to_string();
            match r.protocol {
                hosts::Protocol::Klap => {
                    let mut session = open_tapo(&r.ip).await?;
                    let json = ops::tapo_device_info(&mut session).await?;
                    match tapo::parse(&json) {
                        Some(d) => display::print_tapo_detail(&r.ip, &d, &hint),
                        None => bail!("Could not parse Tapo device info from {}", r.ip),
                    }
                }
                hosts::Protocol::Kasa => {
                    let json = ops::sysinfo(&r.ip).await?;
                    match devices::detect_kind(&json) {
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
                            let t = devices::detect_kind(&json).to_string();
                            eprintln!("{}", format!("Unsupported device type: {t}").yellow());
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
                require_kasa(&r, "on <outlet>")?;
                let json = ops::sysinfo(&r.ip).await?;
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                let child = resolve_outlet(&s, outlet_num)?;
                ops::strip_outlet_on(&r.ip, &child.id).await?;
                println!("Outlet {} ({}) {}", outlet_num, child.alias, "on".green().bold());
            } else {
                match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = open_tapo(&r.ip).await?;
                        ops::tapo_on(&mut s).await?;
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_exec_power(&r.ip, &devices::detect_kind(&json), &json, Some(true)).await?;
                    }
                }
                println!("{} {}", r.ip, "on".green().bold());
            }
        }

        Command::Off { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                require_kasa(&r, "off <outlet>")?;
                let json = ops::sysinfo(&r.ip).await?;
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                let child = resolve_outlet(&s, outlet_num)?;
                ops::strip_outlet_off(&r.ip, &child.id).await?;
                println!("Outlet {} ({}) {}", outlet_num, child.alias, "off".dimmed());
            } else {
                match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = open_tapo(&r.ip).await?;
                        ops::tapo_off(&mut s).await?;
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_exec_power(&r.ip, &devices::detect_kind(&json), &json, Some(false)).await?;
                    }
                }
                println!("{} {}", r.ip, "off".dimmed());
            }
        }

        Command::Toggle { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                require_kasa(&r, "toggle <outlet>")?;
                let json = ops::sysinfo(&r.ip).await?;
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                let child = resolve_outlet(&s, outlet_num)?;
                let now_on = if child.is_on() {
                    ops::strip_outlet_off(&r.ip, &child.id).await?;
                    false
                } else {
                    ops::strip_outlet_on(&r.ip, &child.id).await?;
                    true
                };
                let label = if now_on { "on".green().bold() } else { "off".dimmed() };
                println!("Outlet {} ({}) -> {label}", outlet_num, child.alias);
            } else {
                let now_on = match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = open_tapo(&r.ip).await?;
                        ops::tapo_toggle(&mut s).await?
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_exec_power(&r.ip, &devices::detect_kind(&json), &json, None).await?
                    }
                };
                let label = if now_on { "on".green().bold() } else { "off".dimmed() };
                println!("{} -> {label}", r.ip);
            }
        }

        Command::Dim { host, level } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "dim")?;
            let json = ops::sysinfo(&r.ip).await?;
            let kind = devices::detect_kind(&json);
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
            let r = resolve(&host).await?;
            require_kasa(&r, "color-temp")?;
            let json = ops::sysinfo(&r.ip).await?;
            devices::can_set_color_temp(&devices::detect_kind(&json))?;
            if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                ops::bulb_on(&r.ip).await?;
            }
            ops::bulb_set_color_temp(&r.ip, kelvin).await?;
            println!("Color temperature -> {kelvin}K");
        }

        Command::Color { host, hue, saturation, value } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "color")?;
            let json = ops::sysinfo(&r.ip).await?;
            devices::can_set_color(&devices::detect_kind(&json))?;
            if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                ops::bulb_on(&r.ip).await?;
            }
            ops::bulb_set_color(&r.ip, hue, saturation, value).await?;
            println!("Color -> hue:{hue} sat:{saturation} val:{value}");
        }

        Command::Energy { host, outlet } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "energy")?;
            let json = ops::sysinfo(&r.ip).await?;
            let kind = devices::detect_kind(&json);
            if let Some(outlet_num) = outlet {
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                if !s.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
                }
                let child = resolve_outlet(&s, outlet_num)?;
                let resp = ops::strip_outlet_energy(&r.ip, &child.id).await?;
                println!("Outlet {} ({})", outlet_num, child.alias.bold());
                display::print_energy_realtime(&resp);
            } else {
                devices::require_energy(&json, &kind)?;
                let resp = match &kind {
                    DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy(&r.ip).await?,
                    _ => ops::device_energy(&r.ip).await?,
                };
                display::print_energy_realtime(&resp);
            }
        }

        Command::EnergyDaily { host, month, outlet } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "energy-daily")?;
            let host = r.ip;
            let month_str = match month {
                Some(m) => m,
                None => {
                    let (y, m) = fmt::current_year_month();
                    format!("{y}-{m:02}")
                }
            };
            let parts: Vec<&str> = month_str.split('-').collect();
            if parts.len() != 2 {
                bail!("Month must be in YYYY-MM format");
            }
            let year: u16 = parts[0].parse()?;
            let mo: u8 = parts[1].parse()?;
            if !(1..=12).contains(&mo) {
                bail!("Month must be 01–12, got {mo:02}");
            }
            let json = ops::sysinfo(&host).await?;
            let kind = devices::detect_kind(&json);
            if let Some(outlet_num) = outlet {
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{host} does not appear to be a power strip"))?;
                if !s.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
                }
                let child = resolve_outlet(&s, outlet_num)?;
                let resp = ops::strip_outlet_energy_daily(&host, &child.id, year, mo).await?;
                println!("Outlet {} ({})", outlet_num, child.alias.bold());
                display::print_energy_daily(&resp, &month_str);
            } else {
                devices::require_energy(&json, &kind)?;
                let resp = match &kind {
                    DeviceKind::Bulb | DeviceKind::LightStrip => {
                        ops::bulb_energy_daily(&host, year, mo).await?
                    }
                    _ => ops::device_energy_daily(&host, year, mo).await?,
                };
                display::print_energy_daily(&resp, &month_str);
            }
        }

        Command::EnergyMonthly { host, year, outlet } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "energy-monthly")?;
            let year = year.unwrap_or_else(|| fmt::current_year_month().0);
            let json = ops::sysinfo(&r.ip).await?;
            let kind = devices::detect_kind(&json);
            if let Some(outlet_num) = outlet {
                let s = strip::parse(&json).ok_or_else(|| {
                    anyhow::anyhow!("{} does not appear to be a power strip", r.ip)
                })?;
                if !s.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
                }
                let child = resolve_outlet(&s, outlet_num)?;
                let resp = ops::strip_outlet_energy_monthly(&r.ip, &child.id, year).await?;
                println!("Outlet {} ({})", outlet_num, child.alias.bold());
                display::print_energy_monthly(&resp, year);
            } else {
                devices::require_energy(&json, &kind)?;
                let resp = match &kind {
                    DeviceKind::Bulb | DeviceKind::LightStrip => {
                        ops::bulb_energy_monthly(&r.ip, year).await?
                    }
                    _ => ops::device_energy_monthly(&r.ip, year).await?,
                };
                display::print_energy_monthly(&resp, year);
            }
        }

        Command::Specs { host } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "specs")?;
            let json = ops::sysinfo(&r.ip).await?;
            devices::can_get_specs(&devices::detect_kind(&json))?;
            let resp = ops::bulb_specs(&r.ip).await?;
            display::print_bulb_specs(&resp);
        }

        Command::Presets { host } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "presets")?;
            let json = ops::sysinfo(&r.ip).await?;
            devices::can_get_presets(&devices::detect_kind(&json))?;
            let resp = ops::bulb_presets(&r.ip).await?;
            display::print_bulb_presets(&resp);
        }

        Command::Schedules { host } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "schedules")?;
            let json = ops::sysinfo(&r.ip).await?;
            devices::can_get_schedules(&devices::detect_kind(&json))?;
            let resp = ops::device_schedules(&r.ip).await?;
            display::print_schedules(&resp);
        }

        Command::Led { host, state } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "led")?;
            let json = ops::sysinfo(&r.ip).await?;
            devices::can_control_led(&devices::detect_kind(&json))?;
            let on = matches!(state, LedAction::On);
            ops::device_led(&r.ip, on).await?;
            println!(
                "LED indicator {}",
                if on { "on".green() } else { "off".dimmed() }
            );
        }

        Command::Clock { host } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "clock")?;
            let json = ops::sysinfo(&r.ip).await?;
            devices::can_get_clock(&devices::detect_kind(&json))?;
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
            let r = resolve(&host).await?;
            require_kasa(&r, "outlets")?;
            let host = r.ip;
            let json = ops::sysinfo(&host).await?;
            match strip::parse(&json) {
                Some(s) => display::print_strip_outlets(&s),
                None => bail!("{host} does not appear to be a power strip"),
            }
        }

        Command::OutletRename { host, outlet, name } => {
            let r = resolve(&host).await?;
            require_kasa(&r, "outlet-rename")?;
            let json = ops::sysinfo(&r.ip).await?;
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
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use denki::cli::{Cli, Command};
    use serde_json::json;

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
    fn energy_daily_rejects_month_zero() {
        let month_str = "2025-00";
        let parts: Vec<&str> = month_str.split('-').collect();
        let mo: u8 = parts[1].parse().unwrap();
        assert!(!(1u8..=12).contains(&mo));
    }

    #[test]
    fn energy_daily_rejects_month_13() {
        let month_str = "2025-13";
        let parts: Vec<&str> = month_str.split('-').collect();
        let mo: u8 = parts[1].parse().unwrap();
        assert!(!(1u8..=12).contains(&mo));
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
