use anyhow::{Result, bail};
use colored::Colorize;
use std::net::IpAddr;

use crate::bulb;
use crate::creds;
use crate::devices::{self, DeviceKind};
use crate::dimmer;
use crate::display;
use crate::klap;
use crate::plug;
use crate::strip;

pub(super) async fn tapo_session(ip: &str) -> Result<klap::KlapSession> {
    let (user, pass) = creds::load()?;
    klap::handshake(ip, &user, &pass).await
}

pub(super) fn print_kasa_summary(ip: IpAddr, json: &serde_json::Value, hint: &str) {
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
        DeviceKind::Tapo => display::print_unknown_summary(ip, json, "tapo"),
        DeviceKind::Unknown(t) => display::print_unknown_summary(ip, json, &t),
    }
}

pub(super) fn print_kasa_detail(ip: &str, json: &serde_json::Value, hint: &str) -> Result<()> {
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
