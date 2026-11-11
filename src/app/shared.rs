use crate::output::terminal_text;
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
                terminal_text(&format!(
                    "Detailed info is not available for {kind} yet. Device information from {ip}:"
                ))
                .yellow()
            );
            // Keep identifier redaction consistent with the structured info result.
            let sanitized = crate::output::sanitized(json.clone());
            // JSON escapes data newlines; preserve only the pretty printer's layout.
            for line in serde_json::to_string_pretty(&sanitized)
                .unwrap_or_else(|_| sanitized.to_string())
                .lines()
            {
                crate::output::println!("{}", terminal_text(line));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_detail_fixture() {
        if std::env::var_os("DENKI_UNKNOWN_DETAIL_FIXTURE").is_none() {
            return;
        }
        colored::control::set_override(false);
        let json = serde_json::json!({"system":{"get_sysinfo":{
            "alias":"visible\u{202e}\u{9b}2J",
            "mic_type":"unknown\x1b]52;c;YQ==\x07",
            "model":"unsupported", "mac":"private-mac-sentinel",
            "deviceId":"private-device-sentinel", "ssid":"private-wifi-sentinel"
        }}});
        print_kasa_detail("192.0.2.1", &json, "").unwrap();
        assert_eq!(json["system"]["get_sysinfo"]["mac"], "private-mac-sentinel");
    }

    #[test]
    fn unknown_details_redact_identifiers_and_escape_text() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::shared::tests::unknown_detail_fixture",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("DENKI_UNKNOWN_DETAIL_FIXTURE", "1")
            .output()
            .unwrap();
        assert!(result.status.success());
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert!(stdout.contains("visible\\u{202e}\\u{9b}2J"));
        assert!(!stdout.contains("private-"));
        assert!(
            stdout.contains('\n'),
            "pretty JSON layout should remain readable"
        );
        for text in [&stdout, &String::from_utf8(result.stderr).unwrap()] {
            for c in text.chars().filter(|c| *c != '\n') {
                assert!(crate::output::is_terminal_safe(&c.to_string()), "{text:?}");
            }
        }
    }
}
