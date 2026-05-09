//! Sysinfo types for TP-Link smart power strips (HS300, KP303, KP400).
//!
//! Power strips are detected by the presence of a `children` array in sysinfo.
//! Each child represents one controllable outlet.
//!
//! The parent device controls all outlets at once via set_relay_state.
//! Individual outlets are controlled by passing a `children` array in the
//! command with the target outlet's id and desired state.
//!
//! NOTE: verified = false — not tested on live hardware.

use serde::Deserialize;

/// Top-level sysinfo for a TP-Link smart power strip.
#[derive(Debug, Deserialize)]
pub struct Strip {
    /// Human-readable device name (alias)
    pub alias: String,
    /// Model string, e.g. "HS300(US)"
    pub model: String,
    /// Hardware revision
    pub hw_ver: String,
    /// Firmware version string
    pub sw_ver: String,
    /// Wi-Fi signal strength in dBm
    pub rssi: i32,
    /// Master relay state: 1 = any outlet on, 0 = all off
    pub relay_state: u8,
    /// Capability flags: "TIM", "TIM:ENE"
    pub feature: Option<String>,
    /// Individual outlet states
    #[serde(default)]
    pub children: Vec<StripChild>,
}

/// One controllable outlet on a power strip.
#[derive(Debug, Deserialize)]
pub struct StripChild {
    /// Outlet identifier used for per-outlet commands
    pub id: String,
    /// Human-readable outlet name (alias)
    pub alias: String,
    /// Relay state: 1 = on, 0 = off
    pub state: u8,
    /// Seconds the outlet has been on since last toggle
    #[serde(default)]
    pub on_time: u64,
}

impl Strip {
    pub fn is_any_on(&self) -> bool {
        self.children.iter().any(|c| c.state == 1)
    }

    pub fn has_energy_monitoring(&self) -> bool {
        self.feature
            .as_deref()
            .map(|f| f.contains("ENE"))
            .unwrap_or(false)
    }
}

impl StripChild {
    pub fn is_on(&self) -> bool {
        self.state == 1
    }

    pub fn on_time_fmt(&self) -> String {
        if self.on_time == 0 {
            "off".to_string()
        } else {
            crate::fmt::duration(self.on_time)
        }
    }
}

/// Parse a strip from a raw sysinfo response JSON.
/// Returns None if the sysinfo doesn't contain a `children` array.
pub fn parse(json: &serde_json::Value) -> Option<Strip> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    // Must have children to be a strip
    sysinfo.get("children")?;
    serde_json::from_value(sysinfo.clone()).ok()
}
