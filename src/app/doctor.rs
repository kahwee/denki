use anyhow::Result;
use colored::Colorize;
use serde::Serialize;

use crate::devices;
use crate::hosts;
use crate::ops;
use crate::resolve::resolve_quiet;
use crate::tapo;

use super::shared::tapo_session;

#[derive(Debug, Serialize)]
struct DoctorReport {
    schema_version: u8,
    status: &'static str,
    alias: Option<String>,
    ip: String,
    protocol: String,
    reachable: bool,
    model: Option<String>,
    kind: Option<String>,
    hardware_version: Option<String>,
    firmware_version: Option<String>,
    signal_dbm: Option<i64>,
    power_on: Option<bool>,
    capabilities: Vec<String>,
    verified_model: Option<bool>,
    warnings: Vec<String>,
    error: Option<String>,
}

impl DoctorReport {
    fn pending(alias: Option<String>, ip: String, protocol: &hosts::Protocol) -> Self {
        Self {
            schema_version: 1,
            status: "error",
            alias,
            ip,
            protocol: protocol.to_string(),
            reachable: false,
            model: None,
            kind: None,
            hardware_version: None,
            firmware_version: None,
            signal_dbm: None,
            power_on: None,
            capabilities: Vec::new(),
            verified_model: None,
            warnings: Vec::new(),
            error: None,
        }
    }

    fn apply_registry(&mut self) {
        let Some(model) = self.model.as_deref() else {
            return;
        };
        if let Some(entry) = devices::lookup(model) {
            self.capabilities = entry.supports.clone();
            self.verified_model = Some(entry.verified);
            if !entry.verified {
                self.warnings
                    .push("This model is supported but not verified on physical hardware.".into());
            }
        } else {
            self.warnings.push(format!(
                "Model {model} is not present in the denki capability registry."
            ));
        }
    }
}

fn kasa_report(mut report: DoctorReport, json: &serde_json::Value) -> DoctorReport {
    report.reachable = true;
    let info = json.pointer("/system/get_sysinfo");
    let Some(info) = info else {
        report.error = Some("Response did not contain system.get_sysinfo".into());
        return report;
    };

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

async fn inspect(host: &str) -> Result<DoctorReport> {
    let resolved = resolve_quiet(host)?;
    let mut report = DoctorReport::pending(
        resolved.saved_name.clone(),
        resolved.ip.clone(),
        &resolved.protocol,
    );

    match resolved.protocol {
        hosts::Protocol::Kasa => match ops::sysinfo(&resolved.ip).await {
            Ok(json) => report = kasa_report(report, &json),
            Err(error) => report.error = Some(format!("{error:#}")),
        },
        hosts::Protocol::Klap => match tapo_session(&resolved.ip).await {
            Ok(mut session) => match ops::tapo_device_info(&mut session).await {
                Ok(json) => match tapo::parse(&json) {
                    Some(device) => report = tapo_report(report, &device),
                    None => {
                        report.reachable = true;
                        report.error = Some("Could not parse Tapo device info".into());
                    }
                },
                Err(error) => report.error = Some(format!("{error:#}")),
            },
            Err(error) => report.error = Some(format!("{error:#}")),
        },
    }
    Ok(report)
}

fn print_text(report: &DoctorReport) {
    let label = report.alias.as_deref().unwrap_or(&report.ip);
    println!("{}", format!("Diagnostics: {label}").bold());
    println!("  Host:         {}", report.ip);
    println!("  Protocol:     {}", report.protocol);
    println!(
        "  Reachable:    {}",
        if report.reachable {
            "yes".green().bold()
        } else {
            "no".red().bold()
        }
    );
    if let Some(model) = &report.model {
        println!("  Model:        {model}");
    }
    if let Some(kind) = &report.kind {
        println!("  Type:         {kind}");
    }
    if let Some(firmware) = &report.firmware_version {
        println!("  Firmware:     {firmware}");
    }
    if let Some(on) = report.power_on {
        println!("  Power:        {}", if on { "on" } else { "off" });
    }
    if !report.capabilities.is_empty() {
        println!("  Capabilities: {}", report.capabilities.join(", "));
    }
    for warning in &report.warnings {
        println!("  {} {warning}", "Warning:".yellow());
    }
    if let Some(error) = &report.error {
        println!("  {} {error}", "Error:".red().bold());
    }
}

pub(super) async fn handle_doctor(host: String, json: bool) -> Result<()> {
    let report = inspect(&host).await?;
    let failed = report.status != "ok";
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_text(&report);
    }
    if failed {
        anyhow::bail!("Diagnostics reported a device error")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn kasa_diagnostics_are_structured_and_sanitized() {
        let report = kasa_report(
            DoctorReport::pending(None, "192.0.2.1".into(), &hosts::Protocol::Kasa),
            &json!({"system": {"get_sysinfo": {
                "model": "KL135(US)", "mic_type": "IOT.SMARTBULB",
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
