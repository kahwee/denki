//! Sysinfo for TP-Link HS220 dimmer switches.
//!
//! Detected when IOT.SMARTPLUGSWITCH + "Dimmer" in dev_name.
//! Brightness lives in sysinfo root (not light_state). brightness=0 is invalid
//! hardware — use set_relay_state to turn off instead.
//!
//! NOTE: not tested on live hardware.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Dimmer {
    pub alias: String,
    pub model: String,
    pub hw_ver: String,
    pub sw_ver: String,
    pub rssi: i32,
    pub relay_state: u8,
    #[serde(default)]
    pub brightness: u8,
    pub feature: Option<String>,
}

impl Dimmer {
    pub fn is_on(&self) -> bool {
        self.relay_state == 1
    }
}

pub fn parse(json: &serde_json::Value) -> Option<Dimmer> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    // Structural guard: reject non-switch types (e.g. bulbs). The finer
    // "Dimmer" vs plug/strip discrimination is detect_kind's responsibility.
    let type_str = sysinfo
        .get("mic_type")
        .or_else(|| sysinfo.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !type_str.contains("PLUG") && !type_str.contains("SWITCH") {
        return None;
    }
    serde_json::from_value(sysinfo.clone()).ok()
}
