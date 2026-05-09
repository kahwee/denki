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
    use rstest::rstest;
    use serde_json::json;

    /// Minimal valid Plug for logic-only tests, avoiding JSON parsing overhead.
    fn make_plug(on_time: u64, feature: Option<&str>) -> Plug {
        Plug {
            alias: "Test Plug".to_string(),
            model: "KP115(US)".to_string(),
            hw_ver: "1.0".to_string(),
            sw_ver: "1.0.0".to_string(),
            rssi: -50,
            relay_state: u8::from(on_time > 0),
            on_time,
            led_off: 0,
            feature: feature.map(str::to_string),
        }
    }

    mod parse {
        use super::*;

        #[test]
        fn accepts_newer_mic_type() {
            let json = json!({
                "system": { "get_sysinfo": {
                    "mic_type": "IOT.SMARTPLUGSWITCH",
                    "alias": "Desk Plug", "model": "KP115(US)",
                    "hw_ver": "1.0", "sw_ver": "1.1.1",
                    "rssi": -48, "relay_state": 1,
                    "on_time": 3661, "led_off": 0, "feature": "TIM:ENE"
                }}
            });
            let plug = parse(&json).expect("plug should parse");
            assert!(plug.is_on());
            assert!(plug.has_energy_monitoring());
            assert_eq!(plug.on_time_fmt(), "1h 1m");
        }

        #[test]
        fn accepts_older_type_field() {
            let json = json!({
                "system": { "get_sysinfo": {
                    "type": "IOT.SMARTPLUGSWITCH",
                    "alias": "Old Plug", "model": "HS105(US)",
                    "hw_ver": "5.0", "sw_ver": "1.0.0",
                    "rssi": -70, "relay_state": 0, "feature": "TIM"
                }}
            });
            let plug = parse(&json).expect("older plug should parse");
            assert!(!plug.is_on());
            assert!(!plug.has_energy_monitoring());
        }

        #[test]
        fn rejects_bulb_sysinfo() {
            let json = json!({
                "system": { "get_sysinfo": {
                    "mic_type": "IOT.SMARTBULB", "alias": "Bulb"
                }}
            });
            assert!(parse(&json).is_none());
        }

        #[test]
        fn rejects_missing_sysinfo_wrapper() {
            assert!(parse(&json!({})).is_none());
        }
    }

    mod on_time_display {
        use super::*;

        #[rstest]
        #[case(0,    "off")]
        #[case(1,    "1s")]
        #[case(30,   "30s")]
        #[case(59,   "59s")]
        #[case(60,   "1m 0s")]
        #[case(90,   "1m 30s")]
        #[case(3599, "59m 59s")]
        #[case(3600, "1h 0m")]
        #[case(3661, "1h 1m")]
        #[case(7322, "2h 2m")]
        fn formats_duration(#[case] secs: u64, #[case] expected: &str) {
            assert_eq!(
                make_plug(secs, None).on_time_fmt(),
                expected,
                "on_time={secs}s"
            );
        }
    }

    mod energy_monitoring {
        use super::*;

        #[rstest]
        #[case(Some("TIM:ENE"), true)]
        #[case(Some("ENE"),     true)]
        #[case(Some("TIM"),     false)]
        #[case(Some(""),        false)]
        #[case(None,            false)]
        fn detected_from_feature_string(#[case] feature: Option<&str>, #[case] expected: bool) {
            assert_eq!(
                make_plug(0, feature).has_energy_monitoring(),
                expected,
                "feature={feature:?}"
            );
        }
    }
}
