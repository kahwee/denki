//! Sysinfo types for TP-Link KL135 smart bulbs (mic_type: IOT.SMARTBULB).
//!
//! The KL135 comes in two hardware revisions with different capabilities:
//!
//! HW 1.0 (firmware 1.0.15):
//!   - Basic color, dimming, color temperature
//!   - Energy via smartlife.iot.common.emeter (power_mw + total_wh only)
//!   - No schedule, countdown, or time modules
//!   - No location set (latitude_i = -1879048193 sentinel)
//!
//! HW 2.6 (firmware 1.0.9, built June 2025):
//!   - All HW 1.0 features plus fade_on_off, lnk_on, re_power flags
//!   - get_default_behavior: controls soft_on / hard_on / re_power_type
//!   - Location coordinates are set (has real latitude_i/longitude_i)
//!
//! Neither revision supports: schedule, countdown, time, effects/animations,
//! or LED indicator control. The emeter module returns -2001 — always use
//! smartlife.iot.common.emeter instead.

use serde::Deserialize;

/// Top-level sysinfo for a KL135 smart bulb.
#[derive(Debug, Deserialize)]
pub struct Bulb {
    /// Human-readable device name (alias), e.g. "Office Color 1"
    pub alias: String,
    /// Model string, e.g. "KL135(US)"
    pub model: String,
    /// Hardware revision, e.g. "1.0" or "2.6"
    pub hw_ver: String,
    /// Firmware version string, e.g. "1.0.15 Build 240429 Rel.154143"
    pub sw_ver: String,
    /// Wi-Fi signal strength in dBm. >= -50 excellent, >= -65 good, < -65 weak
    pub rssi: i32,
    /// Device health: "normal" when operating correctly
    #[allow(dead_code)]
    pub dev_state: String,
    /// 1 if the bulb supports full RGB color, 0 otherwise
    #[serde(default)]
    pub is_color: u8,
    /// 1 if the bulb supports brightness adjustment
    #[serde(default)]
    pub is_dimmable: u8,
    /// 1 if the bulb supports color temperature (CCT) mode
    #[serde(default)]
    pub is_variable_color_temp: u8,
    /// Current light state — structure varies depending on whether bulb is on or off
    pub light_state: LightState,
}

/// Light state from sysinfo. The JSON structure differs depending on power state:
///
/// When ON:  brightness/color_temp/hue/saturation are at the top level
/// When OFF: those fields move inside `dft_on_state` (default-on state),
///           and the top-level fields become None
///
/// The accessor methods on this struct hide that complexity.
#[derive(Debug, Deserialize)]
pub struct LightState {
    /// 1 = on, 0 = off
    pub on_off: u8,
    // Inline fields present only when bulb is ON
    pub brightness: Option<u8>,
    pub color_temp: Option<u16>,
    pub hue: Option<u16>,
    pub saturation: Option<u8>,
    /// Present only when bulb is OFF — holds the last-used settings
    pub dft_on_state: Option<DftOnState>,
}

/// Saved light settings used when the bulb is turned off.
/// These are the values that will be restored on next power-on.
#[derive(Debug, Deserialize)]
pub struct DftOnState {
    pub brightness: u8,
    /// 0 means color mode (HSV) is active; > 0 means CCT mode is active
    pub color_temp: u16,
    pub hue: u16,
    pub saturation: u8,
}

impl LightState {
    pub fn is_on(&self) -> bool {
        self.on_off == 1
    }

    /// Brightness 0–100. Falls back to dft_on_state when bulb is off.
    pub fn brightness(&self) -> u8 {
        self.brightness
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.brightness))
            .unwrap_or(0)
    }

    /// Color temperature in Kelvin (2500–9000). 0 means HSV/color mode is active.
    /// Falls back to dft_on_state when bulb is off.
    pub fn color_temp(&self) -> u16 {
        self.color_temp
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.color_temp))
            .unwrap_or(0)
    }

    /// Hue 0–360 degrees. Only meaningful when color_temp() == 0.
    pub fn hue(&self) -> u16 {
        self.hue
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.hue))
            .unwrap_or(0)
    }

    /// Saturation 0–100. Only meaningful when color_temp() == 0.
    pub fn saturation(&self) -> u8 {
        self.saturation
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.saturation))
            .unwrap_or(0)
    }
}

/// Parse a bulb from a raw sysinfo response JSON.
/// Returns None if the JSON doesn't match the expected shape.
pub fn parse(json: &serde_json::Value) -> Option<Bulb> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    serde_json::from_value(sysinfo.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Minimal valid sysinfo JSON for a KL135 bulb.
    fn bulb_json(on_off: u8, brightness: u8, color_temp: u16, hue: u16, sat: u8) -> serde_json::Value {
        json!({
            "system": { "get_sysinfo": {
                "alias": "Test Bulb", "model": "KL135(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0",
                "rssi": -55, "dev_state": "normal",
                "light_state": {
                    "on_off": on_off,
                    "brightness": brightness,
                    "color_temp": color_temp,
                    "hue": hue,
                    "saturation": sat
                }
            }}
        })
    }

    #[test]
    fn parse_reads_inline_light_state_when_on() {
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "alias": "Office Color",
                    "model": "KL135(US)",
                    "hw_ver": "2.6",
                    "sw_ver": "1.0.9 Build 250610 Rel.123456",
                    "rssi": -52,
                    "dev_state": "normal",
                    "is_color": 1,
                    "is_dimmable": 1,
                    "is_variable_color_temp": 1,
                    "light_state": {
                        "on_off": 1,
                        "brightness": 80,
                        "color_temp": 2700,
                        "hue": 0,
                        "saturation": 0
                    }
                }
            }
        });

        let bulb = parse(&json).expect("bulb should parse");

        assert!(bulb.light_state.is_on());
        assert_eq!(bulb.alias, "Office Color");
        assert_eq!(bulb.light_state.brightness(), 80);
        assert_eq!(bulb.light_state.color_temp(), 2700);
    }

    #[test]
    fn parse_falls_back_to_default_on_state_when_off() {
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "alias": "Office Color",
                    "model": "KL135(US)",
                    "hw_ver": "1.0",
                    "sw_ver": "1.0.15 Build 240429 Rel.154143",
                    "rssi": -61,
                    "dev_state": "normal",
                    "is_color": 1,
                    "is_dimmable": 1,
                    "is_variable_color_temp": 1,
                    "light_state": {
                        "on_off": 0,
                        "dft_on_state": {
                            "brightness": 45,
                            "color_temp": 0,
                            "hue": 275,
                            "saturation": 50
                        }
                    }
                }
            }
        });

        let bulb = parse(&json).expect("bulb should parse");

        assert!(!bulb.light_state.is_on());
        assert_eq!(bulb.light_state.brightness(), 45);
        assert_eq!(bulb.light_state.color_temp(), 0);
        assert_eq!(bulb.light_state.hue(), 275);
        assert_eq!(bulb.light_state.saturation(), 50);
    }

    #[test]
    fn parse_rejects_plug_sysinfo_missing_light_state() {
        // Plug sysinfo has no light_state — deserialization must fail cleanly.
        let json = json!({
            "system": { "get_sysinfo": {
                "mic_type": "IOT.SMARTPLUGSWITCH",
                "alias": "Desk Plug", "model": "KP115(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0",
                "rssi": -50, "relay_state": 1
            }}
        });
        assert!(parse(&json).is_none());
    }

    #[test]
    fn parse_returns_none_for_empty_json() {
        assert!(parse(&json!({})).is_none());
    }

    #[test]
    fn fixture_helper_produces_parseable_bulb() {
        let bulb = parse(&bulb_json(1, 75, 4000, 0, 0)).expect("fixture should parse");
        assert!(bulb.light_state.is_on());
        assert_eq!(bulb.light_state.brightness(), 75);
        assert_eq!(bulb.light_state.color_temp(), 4000);
    }

    #[test]
    fn color_mode_active_when_color_temp_is_zero() {
        // sat > 0 + color_temp = 0 → HSV/color mode
        let bulb = parse(&bulb_json(1, 80, 0, 120, 100)).expect("should parse");
        assert_eq!(bulb.light_state.color_temp(), 0);
        assert_eq!(bulb.light_state.hue(), 120);
        assert_eq!(bulb.light_state.saturation(), 100);
    }
}
