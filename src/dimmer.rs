//! Sysinfo types for TP-Link smart dimmer switches (HS220).
//!
//! Dimmers are detected when the type field is IOT.SMARTPLUGSWITCH and
//! `dev_name` contains "Dimmer". They extend plug behaviour with a
//! brightness level stored in sysinfo and a dedicated dimmer service
//! for setting brightness or fade transitions.
//!
//! Key differences from a plain plug:
//!   - Brightness is in sysinfo root (not inside light_state like a bulb)
//!   - Power/brightness via: smartlife.iot.dimmer / set_brightness
//!   - set_dimmer_transition supports fade-to-brightness over milliseconds
//!   - brightness=0 is not valid; use set_relay_state to turn off instead
//!   - May have PIR motion (smartlife.iot.PIR) and ambient light (smartlife.iot.LAS)
//!     sensors depending on hardware variant
//!
//! NOTE: verified = false — not tested on live hardware.

use serde::Deserialize;

/// Top-level sysinfo for a TP-Link smart dimmer switch.
#[derive(Debug, Deserialize)]
pub struct Dimmer {
    /// Human-readable device name (alias)
    pub alias: String,
    /// Model string, e.g. "HS220(US)"
    pub model: String,
    /// Hardware revision
    pub hw_ver: String,
    /// Firmware version string
    pub sw_ver: String,
    /// Wi-Fi signal strength in dBm
    pub rssi: i32,
    /// Relay state: 1 = on, 0 = off
    pub relay_state: u8,
    /// Current brightness level 0–100 (in sysinfo root, unlike bulbs)
    #[serde(default)]
    pub brightness: u8,
    /// Capability flags
    pub feature: Option<String>,
}

impl Dimmer {
    pub fn is_on(&self) -> bool {
        self.relay_state == 1
    }
}

/// Parse a dimmer from a raw sysinfo response JSON.
/// Returns None if dev_name doesn't contain "Dimmer".
pub fn parse(json: &serde_json::Value) -> Option<Dimmer> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    let dev_name = sysinfo
        .get("dev_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !dev_name.contains("Dimmer") {
        return None;
    }
    serde_json::from_value(sysinfo.clone()).ok()
}
