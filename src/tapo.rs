//! Sysinfo types for Tapo smart devices (P125, etc.) using KLAP protocol.
//!
//! Tapo device info is fetched with:
//!   {"method": "get_device_info", "params": {}}
//!
//! Response wraps in: {"error_code": 0, "result": {...}}
//!
//! Key differences from legacy IoT devices:
//!   - `nickname` is base64-encoded
//!   - `device_on` (bool) instead of relay_state (0/1)
//!   - `ssid` is base64-encoded
//!   - No `feature` string — all Tapo plugs support energy where the hardware has it
//!
//! Power control:
//!   {"method": "set_device_info", "params": {"device_on": true}}
//!   {"method": "set_device_info", "params": {"device_on": false}}

use serde::Deserialize;

/// Device info returned by `get_device_info` for a Tapo smart plug.
#[derive(Debug, Deserialize)]
pub struct TapoDevice {
    /// Device model, e.g. "P125M"
    pub model: String,
    /// Hardware version, e.g. "1.0"
    pub hw_ver: String,
    /// Firmware version string
    pub fw_ver: String,
    /// Current relay state
    pub device_on: bool,
    /// Seconds the device has been on since last toggle
    #[serde(default)]
    pub on_time: u64,
    /// Wi-Fi signal strength in dBm
    pub rssi: i32,
    /// Signal quality level 0–3
    #[serde(default)]
    pub signal_level: u8,
    /// Device nickname (base64-encoded in response — decoded on parse)
    #[serde(default)]
    pub nickname: String,
    /// Whether the device is overheating
    #[serde(default)]
    pub overheated: bool,
    /// Unique device ID
    pub device_id: String,
}

impl TapoDevice {
    pub fn is_on(&self) -> bool {
        self.device_on
    }
}

/// Parse a TapoDevice from the full `get_device_info` response JSON.
/// Decodes the base64-encoded nickname field.
pub fn parse(json: &serde_json::Value) -> Option<TapoDevice> {
    let result = json.get("result")?;
    let mut device: TapoDevice = serde_json::from_value(result.clone()).ok()?;

    // Nickname is base64-encoded in the API response
    if let Ok(bytes) = BASE64.decode(device.nickname.as_bytes()) {
        if let Ok(s) = String::from_utf8(bytes) {
            device.nickname = s;
        }
    }

    Some(device)
}

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
