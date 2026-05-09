//! Sysinfo types for TP-Link KP115 smart plugs (mic_type: IOT.SMARTPLUGSWITCH).
//!
//! The KP115 is a mini smart plug with energy monitoring. Key differences
//! from the KL135 bulb:
//!
//! - Power controlled via relay_state (not lightingservice)
//! - Full energy data: voltage (mv), current (ma), power (mw), total (wh)
//! - Supports schedule rules, device time, LED indicator control
//! - Does NOT support brightness, color, or color temperature
//! - Does NOT support countdown timers (returns -1 not supported)
//!
//! Feature string in sysinfo encodes capabilities:
//!   "TIM" = timer/schedule support
//!   "ENE" = energy monitoring
//!   "TIM:ENE" = both (typical for KP115)

use serde::Deserialize;

/// Top-level sysinfo for a KP115 smart plug.
#[derive(Debug, Deserialize)]
pub struct Plug {
    /// Human-readable device name (alias), e.g. "Living Room Right Lamp"
    pub alias: String,
    /// Model string, e.g. "KP115(US)"
    pub model: String,
    /// Hardware revision, e.g. "1.0"
    pub hw_ver: String,
    /// Firmware version string, e.g. "1.1.1 Build 250908 Rel.112945"
    pub sw_ver: String,
    /// Wi-Fi signal strength in dBm. >= -50 excellent, >= -65 good, < -65 weak
    pub rssi: i32,
    /// Relay (outlet) state: 1 = on, 0 = off
    pub relay_state: u8,
    /// Seconds the relay has been on since last toggle. 0 when off.
    #[serde(default)]
    pub on_time: u64,
    /// LED indicator state: 1 = LED off, 0 = LED on (inverted — "off" means disabled)
    #[serde(default)]
    pub led_off: u8,
    /// Capability flags: "TIM" (timer), "ENE" (energy), "TIM:ENE" (both)
    pub feature: Option<String>,
}

impl Plug {
    /// Whether the outlet relay is currently switched on.
    pub fn is_on(&self) -> bool {
        self.relay_state == 1
    }

    /// Whether this plug has an energy monitoring chip.
    /// Detected from the "ENE" token in the feature string.
    /// KP115 always has this; simpler plugs like HS103 do not.
    pub fn has_energy_monitoring(&self) -> bool {
        self.feature
            .as_deref()
            .map(|f| f.contains("ENE"))
            .unwrap_or(false)
    }

    /// Format on_time seconds as a human-readable duration string.
    /// Returns "off" when on_time is 0 (relay is off).
    pub fn on_time_fmt(&self) -> String {
        if self.on_time == 0 {
            return "off".to_string();
        }
        let h = self.on_time / 3600;
        let m = (self.on_time % 3600) / 60;
        let s = self.on_time % 60;
        if h > 0 {
            format!("{h}h {m}m")
        } else if m > 0 {
            format!("{m}m {s}s")
        } else {
            format!("{s}s")
        }
    }
}

/// Parse a plug from a raw sysinfo response JSON.
///
/// Only returns Some if mic_type contains "PLUG" or "SWITCH" — this guards
/// against accidentally parsing a bulb response as a plug, since both share
/// the same sysinfo wrapper.
pub fn parse(json: &serde_json::Value) -> Option<Plug> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    // Newer devices (KP115) use `mic_type`; older devices (HS110) use `type`
    let type_str = sysinfo
        .get("mic_type")
        .or_else(|| sysinfo.get("type"))
        .and_then(|v| v.as_str())?;
    if !type_str.contains("PLUG") && !type_str.contains("SWITCH") {
        return None;
    }
    serde_json::from_value(sysinfo.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_accepts_newer_plug_mic_type() {
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "mic_type": "IOT.SMARTPLUGSWITCH",
                    "alias": "Desk Plug",
                    "model": "KP115(US)",
                    "hw_ver": "1.0",
                    "sw_ver": "1.1.1 Build 250908 Rel.112945",
                    "rssi": -48,
                    "relay_state": 1,
                    "on_time": 3661,
                    "led_off": 0,
                    "feature": "TIM:ENE"
                }
            }
        });

        let plug = parse(&json).expect("plug should parse");

        assert!(plug.is_on());
        assert!(plug.has_energy_monitoring());
        assert_eq!(plug.on_time_fmt(), "1h 1m");
    }

    #[test]
    fn parse_accepts_older_type_field() {
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "type": "IOT.SMARTPLUGSWITCH",
                    "alias": "Old Plug",
                    "model": "HS105(US)",
                    "hw_ver": "5.0",
                    "sw_ver": "1.0.0",
                    "rssi": -70,
                    "relay_state": 0,
                    "feature": "TIM"
                }
            }
        });

        let plug = parse(&json).expect("older plug should parse");

        assert!(!plug.is_on());
        assert!(!plug.has_energy_monitoring());
        assert_eq!(plug.on_time_fmt(), "off");
    }

    #[test]
    fn parse_rejects_non_plug_device() {
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "mic_type": "IOT.SMARTBULB",
                    "alias": "Bulb"
                }
            }
        });

        assert!(parse(&json).is_none());
    }
}
