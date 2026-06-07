use denki::cli::{Cli, Command};
use denki::devices::{self, DeviceKind};
use denki::resolve::resolve_quiet;
use denki::{admin, commands};
use denki::{
    bulb, creds, dimmer, display, effects, hosts, klap, ops, plug, strip, tapo, transport,
};

use anyhow::{bail, Result};
use clap::Parser;
use colored::Colorize;
use std::net::IpAddr;

async fn tapo_session(ip: &str) -> Result<klap::KlapSession> {
    let (user, pass) = creds::load()?;
    klap::handshake(ip, &user, &pass).await
}

fn print_kasa_summary(ip: IpAddr, json: &serde_json::Value, hint: &str) {
    match devices::detect_kind(json) {
        DeviceKind::Bulb => {
            if let Some(b) = bulb::parse(json) {
                display::print_bulb_summary(ip, &b, hint);
            }
        }
        DeviceKind::LightStrip => {
            if let Some(b) = bulb::parse(json) {
                display::print_lightstrip_summary(ip, &b, hint);
            }
        }
        DeviceKind::Dimmer => {
            if let Some(d) = dimmer::parse(json) {
                display::print_dimmer_summary(ip, &d, hint);
            }
        }
        DeviceKind::Strip => {
            if let Some(s) = strip::parse(json) {
                display::print_strip_summary(ip, &s, hint);
            }
        }
        DeviceKind::Plug => {
            if let Some(p) = plug::parse(json) {
                display::print_plug_summary(ip, &p, hint);
            }
        }
        // Tapo devices use KLAP on port 80 and won't respond to UDP probe.
        DeviceKind::Tapo => display::print_unknown_summary(ip, json, "tapo"),
        DeviceKind::Unknown(t) => display::print_unknown_summary(ip, json, &t),
    }
}

fn print_kasa_detail(ip: &str, json: &serde_json::Value, hint: &str) -> Result<()> {
    let kind = devices::detect_kind(json);
    match kind {
        DeviceKind::Bulb => match bulb::parse(json) {
            Some(b) => display::print_bulb_detail(ip, &b, hint),
            None => bail!("Could not parse bulb sysinfo from {}", ip),
        },
        DeviceKind::LightStrip => match bulb::parse(json) {
            Some(b) => display::print_lightstrip_detail(ip, &b, hint),
            None => bail!("Could not parse light strip sysinfo from {}", ip),
        },
        DeviceKind::Dimmer => match dimmer::parse(json) {
            Some(d) => display::print_dimmer_detail(ip, &d, hint),
            None => bail!("Could not parse dimmer sysinfo from {}", ip),
        },
        DeviceKind::Strip => match strip::parse(json) {
            Some(s) => display::print_strip_detail(ip, &s, hint),
            None => bail!("Could not parse strip sysinfo from {}", ip),
        },
        DeviceKind::Plug => match plug::parse(json) {
            Some(p) => display::print_plug_detail(ip, &p, hint),
            None => bail!("Could not parse plug sysinfo from {}", ip),
        },
        DeviceKind::Tapo | DeviceKind::Unknown(_) => {
            eprintln!(
                "{}",
                format!("Detailed info is not available for {kind} yet. Raw sysinfo from {ip}:")
                    .yellow()
            );
            println!(
                "{}",
                serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
            );
        }
    }
    Ok(())
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
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|name| hosts::save_if_new_in(name, &ip_str, &mut host_map));
                if is_new {
                    map_dirty = true;
                }
                let hint =
                    hosts::lookup_by_ip_in(&ip_str, &host_map).unwrap_or_else(|| ip_str.clone());
                print_kasa_summary(ip, &json, &hint);
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
                    print_kasa_detail(&r.ip, &json, &hint)?;
                }
            }
        }

        Command::On { host, outlet } => {
            commands::handle_on(&host, outlet).await?;
        }

        Command::Off { host, outlet } => {
            commands::handle_off(&host, outlet).await?;
        }

        Command::Toggle { host, outlet } => {
            commands::handle_toggle(&host, outlet).await?;
        }

        Command::Dim { host, level } => {
            commands::handle_dim(&host, level).await?;
        }

        Command::ColorTemp { host, kelvin } => {
            commands::handle_color_temp(&host, kelvin).await?;
        }

        Command::Color {
            host,
            hue,
            saturation,
            value,
        } => {
            commands::handle_color(&host, hue, saturation, value).await?;
        }

        Command::Energy { host, outlet } => {
            commands::handle_energy(&host, outlet).await?;
        }

        Command::EnergyDaily {
            host,
            month,
            outlet,
        } => {
            commands::handle_energy_daily(&host, month, outlet).await?;
        }

        Command::EnergyMonthly { host, year, outlet } => {
            commands::handle_energy_monthly(&host, year, outlet).await?;
        }

        Command::Specs { host } => {
            admin::handle_specs(&host).await?;
        }

        Command::Presets { host } => {
            admin::handle_presets(&host).await?;
        }

        Command::Effects { host } => effects::handle_effects_command(&host).await?,

        Command::Effect { host, name } => effects::handle_effect_command(&host, &name).await?,

        Command::Schedules { host } => {
            admin::handle_schedules(&host).await?;
        }

        Command::Led { host, state } => {
            let on = matches!(state, denki::cli::LedAction::On);
            admin::handle_led(&host, on).await?;
        }

        Command::Clock { host } => {
            admin::handle_clock(&host).await?;
        }

        Command::Rename { host, name } => {
            admin::handle_rename(&host, &name).await?;
        }

        Command::Restart { host } => {
            admin::handle_restart(&host).await?;
        }

        Command::Outlets { host } => {
            admin::handle_outlets(&host).await?;
        }

        Command::OutletRename { host, outlet, name } => {
            admin::handle_outlet_rename(&host, outlet, &name).await?;
        }

        Command::Alias { name, ip, klap } => {
            admin::handle_alias(&name, &ip, klap)?;
        }

        Command::Unalias { name } => {
            admin::handle_unalias(&name)?;
        }

        Command::Aliases => {
            admin::handle_aliases()?;
        }

        Command::Login { email, password } => {
            admin::handle_login(&email, password)?;
        }

        Command::Completions { shell } => {
            admin::handle_completions(shell);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        "power",
        "dim",
        "color_temp",
        "color",
        "specs",
        "presets",
        "schedules",
        "led",
        "clock",
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
            "energy" | "outlets" | "effects" => Ok(()), // runtime-checked, no static guard
            other => panic!(
                "devices.toml: unknown feature '{other}' — add it to check() or explain \
                 why it has no guard"
            ),
        }
    }

    #[test]
    fn listed_features_are_permitted_by_guards() {
        for dev in devices::all() {
            let Some(kind) = guard_kind(&dev.kind) else {
                continue;
            };
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
            let Some(kind) = guard_kind(&dev.kind) else {
                continue;
            };
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
