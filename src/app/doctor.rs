use crate::output::terminal_text;
use anyhow::Result;
use colored::Colorize;

use crate::devices;
use crate::hosts;
use crate::ops;
use crate::resolve::resolve_quiet;
use crate::tapo;

use super::shared::tapo_session;

mod report;
use report::DoctorReport;
mod fleet;

fn kasa_report(mut report: DoctorReport, json: &serde_json::Value) -> DoctorReport {
    report.reachable = true;
    let info = json.pointer("/system/get_sysinfo");
    let Some(info) = info else {
        report.error = Some("Response did not contain system.get_sysinfo".into());
        report.error_code = Some("malformed_response");
        return report;
    };

    if let Err(error) = ops::validate_sysinfo(json) {
        report.error_code = Some(crate::error::code(&error));
        report.error = Some(error.to_string());
        return report;
    }
    report.status = "ok";
    report.model = info
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let kind = devices::detect_kind(json);
    report.kind = Some(kind.to_string());
    report.power_on = match kind {
        devices::DeviceKind::Bulb | devices::DeviceKind::LightStrip => {
            crate::bulb::parse(json).map(|device| device.light_state.is_on())
        }
        devices::DeviceKind::Strip => crate::strip::parse(json).map(|device| device.is_any_on()),
        _ => info
            .get("relay_state")
            .and_then(|value| value.as_u64())
            .map(|state| state == 1),
    };
    report.hardware_version = info
        .get("hw_ver")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    report.firmware_version = info
        .get("sw_ver")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    report.signal_dbm = info.get("rssi").and_then(|v| v.as_i64());
    report.apply_registry();
    report
}

fn tapo_report(mut report: DoctorReport, device: &tapo::TapoDevice) -> DoctorReport {
    report.reachable = true;
    report.status = "ok";
    report.model = Some(device.model.clone());
    report.kind = Some("tapo".into());
    report.hardware_version = Some(device.hw_ver.clone());
    report.firmware_version = Some(device.fw_ver.clone());
    report.signal_dbm = Some(i64::from(device.rssi));
    report.power_on = Some(device.device_on);
    report.apply_registry();
    if device.overheated {
        report
            .warnings
            .push("Device reports an overheated state.".into());
    }
    report
}

async fn inspect_resolved(resolved: crate::resolve::Resolved) -> Result<DoctorReport> {
    let mut report = DoctorReport::pending(
        resolved.saved_name.clone(),
        resolved.ip.clone(),
        &resolved.protocol,
    );

    match resolved.protocol {
        hosts::Protocol::Tapo => match crate::tapo_client::info(&resolved.ip).await {
            Ok(device) => report = tapo_report(report, &device),
            Err(error) => report.record_failure(&error),
        },
        hosts::Protocol::Kasa => match ops::sysinfo(&resolved.ip).await {
            Ok(json) => report = kasa_report(report, &json),
            Err(error) => report.record_failure(&error),
        },
        hosts::Protocol::Klap => match tapo_session(&resolved.ip).await {
            Ok(mut session) => match ops::tapo_device_info(&mut session).await {
                Ok(json) => match tapo::parse(&json) {
                    Some(device) => report = tapo_report(report, &device),
                    None => {
                        report.reachable = true;
                        report.error_code = Some("malformed_response");
                        report.error = Some("Could not parse Tapo device info".into());
                    }
                },
                Err(error) => {
                    report.reachable = true; // The KLAP handshake already established connectivity.
                    report.record_failure(&error);
                }
            },
            Err(error) => report.record_failure(&error),
        },
    }
    report.add_guidance();
    Ok(report)
}

fn print_text(report: &DoctorReport) {
    let label = terminal_text(report.alias.as_deref().unwrap_or(&report.ip));
    crate::output::println!("{}", format!("Diagnostics: {label}").bold());
    crate::output::println!("  Host:         {}", terminal_text(&report.ip));
    crate::output::println!("  Protocol:     {}", report.protocol);
    crate::output::println!(
        "  Reachable:    {}",
        if report.reachable {
            "yes".green().bold()
        } else {
            "no".red().bold()
        }
    );
    crate::output::println!(
        "  Identification: {}",
        if report.physically_confirmed {
            "physically confirmed by user (saved declaration)"
        } else {
            "not physically confirmed"
        }
    );
    if let Some(model) = &report.model {
        crate::output::println!("  Model:        {}", terminal_text(model));
    }
    if let Some(kind) = &report.kind {
        crate::output::println!("  Type:         {}", terminal_text(kind));
    }
    if let Some(firmware) = &report.firmware_version {
        crate::output::println!("  Firmware:     {}", terminal_text(firmware));
    }
    if let Some(signal) = report.signal_dbm {
        crate::output::println!("  Wi-Fi RSSI:   {signal} dBm");
    }
    if let Some(on) = report.power_on {
        crate::output::println!("  Power:        {}", if on { "on" } else { "off" });
    }
    if !report.capabilities.is_empty() {
        crate::output::println!(
            "  Capabilities: {}",
            terminal_text(&report.capabilities.join(", "))
        );
    }
    for warning in &report.warnings {
        crate::output::println!("  {} {}", "Warning:".yellow(), terminal_text(warning));
    }
    for step in &report.next_steps {
        crate::output::println!("  Next step:    {}", terminal_text(step));
    }
    if let Some(error) = &report.error {
        crate::output::println!("  {} {}", "Error:".red().bold(), terminal_text(error));
    }
}

pub(super) async fn handle_doctor(
    host: Option<String>,
    all: bool,
    failed_only: bool,
    concurrency: u8,
    timeout: u16,
) -> Result<()> {
    if all {
        return fleet::run(concurrency, timeout, failed_only).await;
    }
    let host =
        host.ok_or_else(|| crate::error::error("invalid_arguments", "Provide DEVICE or --all"))?;
    let resolved = resolve_quiet(&host)?;
    let entry = hosts::lookup(&host)?;
    let mut report = fleet::bounded_inspect(resolved, timeout).await;
    report.physically_confirmed = entry.is_some_and(|e| e.physically_confirmed);
    let failed = report.status != "ok";
    crate::output::record(serde_json::to_value(&report)?);
    print_text(&report);
    if failed {
        return Err(crate::error::error(
            report.error_code.unwrap_or("command_failed"),
            report
                .error
                .unwrap_or_else(|| "Diagnostics reported a device error".into()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn auto_adapter_does_not_inherit_klap_verification() {
        let mut report = DoctorReport::pending(None, "192.0.2.1".into(), &hosts::Protocol::Tapo);
        report.model = Some("P125".into());
        report.apply_registry();
        assert_eq!(report.verified_model, Some(false));
        assert_eq!(report.capabilities, vec!["power"]);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("wire protocol"))
        );
        report.model = Some("P110".into());
        report.apply_registry();
        assert!(report.capabilities.is_empty());
    }

    #[test]
    fn kasa_diagnostics_are_structured_and_sanitized() {
        let report = kasa_report(
            DoctorReport::pending(None, "192.0.2.1".into(), &hosts::Protocol::Kasa),
            &json!({"system": {"get_sysinfo": {
                "alias":"Test", "err_code": 0, "model": "KL135(US)", "mic_type": "IOT.SMARTBULB",
                "hw_ver": "1.0", "sw_ver": "1.2.3", "rssi": -44,
                "deviceId": "secret-device-id", "mac": "00:11:22:33:44:55",
                "light_state": {"on_off": 1}
            }}}),
        );
        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["status"], "ok");
        assert_eq!(value["model"], "KL135(US)");
        assert!(value["capabilities"].as_array().unwrap().len() > 1);
        let serialized = value.to_string();
        assert!(!serialized.contains("secret-device-id"));
        assert!(!serialized.contains("00:11:22:33:44:55"));
    }

    #[test]
    fn malformed_kasa_response_is_reported_without_panicking() {
        let report = kasa_report(
            DoctorReport::pending(None, "192.0.2.2".into(), &hosts::Protocol::Kasa),
            &json!({"unexpected": true}),
        );
        assert!(report.reachable);
        assert!(report.error.unwrap().contains("get_sysinfo"));
    }
}
