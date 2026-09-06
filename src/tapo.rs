//! Tapo device info (P125, etc.) via KLAP get_device_info.
//! Response shape: {"error_code": 0, "result": {...}}
//! nickname and ssid are base64-encoded in the API response.

use anyhow::{Context, Result};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct TapoDevice {
    pub model: String,
    pub hw_ver: String,
    pub fw_ver: String,
    pub device_on: bool,
    #[serde(default)]
    pub on_time: u64,
    pub rssi: i32,
    #[serde(default)]
    pub signal_level: u8,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub overheated: bool,
    pub device_id: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct TapoEnergyUsage {
    /// Instantaneous power in milliwatts.
    pub current_power: Option<u64>,
    /// Energy consumed today in watt-hours.
    pub today_energy: Option<u64>,
    /// Energy consumed this month in watt-hours.
    pub month_energy: Option<u64>,
    /// Runtime today in minutes.
    pub today_runtime: Option<u64>,
    /// Runtime this month in minutes.
    pub month_runtime: Option<u64>,
}

impl TapoDevice {
    pub fn is_on(&self) -> bool {
        self.device_on
    }
}

pub fn parse(json: &serde_json::Value) -> Option<TapoDevice> {
    let result = json.get("result")?;
    let mut device: TapoDevice = serde_json::from_value(result.clone()).ok()?;

    // Nickname is base64-encoded in the API response
    if let Ok(bytes) = BASE64.decode(device.nickname.as_bytes())
        && let Ok(s) = String::from_utf8(bytes)
    {
        device.nickname = s;
    }

    Some(device)
}

pub fn parse_energy_usage(json: &serde_json::Value) -> Result<TapoEnergyUsage> {
    let result = json
        .get("result")
        .cloned()
        .context("Tapo energy response did not contain a result")?;
    serde_json::from_value(result).context("Could not parse Tapo energy usage")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_decodes_base64_nickname() {
        let json = json!({
            "error_code": 0,
            "result": {
                "model": "P125M",
                "hw_ver": "1.0",
                "fw_ver": "1.2.3 Build 260101 Rel.123456",
                "device_on": true,
                "on_time": 42,
                "rssi": -55,
                "signal_level": 2,
                "nickname": "RGVzayBQbHVn",
                "overheated": false,
                "device_id": "abc123"
            }
        });

        let device = parse(&json).expect("tapo device should parse");

        assert_eq!(device.nickname, "Desk Plug");
        assert!(device.is_on());
        assert_eq!(device.signal_level, 2);
    }

    #[test]
    fn parse_leaves_plain_nickname_when_base64_decode_fails() {
        let json = json!({
            "result": {
                "model": "P125M",
                "hw_ver": "1.0",
                "fw_ver": "1.2.3",
                "device_on": false,
                "rssi": -75,
                "nickname": "not base64!",
                "device_id": "abc123"
            }
        });

        let device = parse(&json).expect("tapo device should parse");

        assert_eq!(device.nickname, "not base64!");
        assert!(!device.is_on());
        assert_eq!(device.signal_level, 0);
    }

    #[test]
    fn parse_energy_usage_preserves_milliwatts_and_watt_hours() {
        let usage = parse_energy_usage(&json!({
            "error_code": 0,
            "result": {
                "current_power": 1234,
                "today_energy": 56,
                "month_energy": 789,
                "today_runtime": 12,
                "month_runtime": 345
            }
        }))
        .unwrap();

        assert_eq!(usage.current_power, Some(1234));
        assert_eq!(usage.today_energy, Some(56));
        assert_eq!(usage.month_energy, Some(789));
    }
}
