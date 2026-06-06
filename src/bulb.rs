//! Sysinfo for TP-Link KL135/LB130 bulbs and KL430 light strips (mic_type: IOT.SMARTBULB).
//!
//! HW 1.0 (FW 1.0.15): basic color, dim, CCT. Energy via smartlife.iot.common.emeter only
//!   (bare "emeter" returns -2001). No schedule, countdown, time, or LED control.
//! HW 2.6 (FW 1.0.9+): same plus fade_on_off, lnk_on, re_power, get_default_behavior.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Bulb {
    pub alias: String,
    pub model: String,
    pub hw_ver: String,
    pub sw_ver: String,
    pub rssi: i32,
    #[serde(default)]
    pub is_color: u8,
    #[serde(default)]
    pub is_dimmable: u8,
    #[serde(default)]
    pub is_variable_color_temp: u8,
    pub light_state: LightState,
    #[serde(default)]
    pub lighting_effect_state: Option<LightingEffectState>,
}

/// When ON:  brightness/color_temp/hue/saturation are at the top level.
/// When OFF: those fields move inside `dft_on_state`; top-level fields become None.
/// Accessor methods hide this.
#[derive(Debug, Deserialize)]
pub struct LightState {
    pub on_off: u8,
    pub brightness: Option<u8>,
    pub color_temp: Option<u16>,
    pub hue: Option<u16>,
    pub saturation: Option<u8>,
    pub dft_on_state: Option<DftOnState>,
}

#[derive(Debug, Deserialize)]
pub struct DftOnState {
    pub brightness: u8,
    // 0 = HSV/color mode active; > 0 = CCT mode active
    pub color_temp: u16,
    pub hue: u16,
    pub saturation: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LightingEffectState {
    #[serde(default)]
    pub enable: u8,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub custom: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion_strategy: Option<u8>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub effect_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue_range: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation_range: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness_range: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_range: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_states: Option<Vec<Vec<u8>>>,
}

impl LightState {
    pub fn is_on(&self) -> bool {
        self.on_off == 1
    }

    pub fn brightness(&self) -> u8 {
        self.brightness
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.brightness))
            .unwrap_or(0)
    }

    pub fn color_temp(&self) -> u16 {
        self.color_temp
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.color_temp))
            .unwrap_or(0)
    }

    pub fn hue(&self) -> u16 {
        self.hue
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.hue))
            .unwrap_or(0)
    }

    pub fn saturation(&self) -> u8 {
        self.saturation
            .or_else(|| self.dft_on_state.as_ref().map(|s| s.saturation))
            .unwrap_or(0)
    }
}

pub fn parse(json: &serde_json::Value) -> Option<Bulb> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    serde_json::from_value(sysinfo.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bulb_json(
        on_off: u8,
        brightness: u8,
        color_temp: u16,
        hue: u16,
        sat: u8,
    ) -> serde_json::Value {
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
    fn lighting_effect_state_parses_full_descriptor() {
        let json = json!({
            "enable": 1,
            "name": "Rainbow",
            "custom": 0,
            "id": "TapoStrip_4HVKmMc6vEzjm36jXaGwMs",
            "brightness": 100,
            "segments": [1],
            "expansion_strategy": 1,
            "type": "random",
            "hue_range": [30, 40],
            "saturation_range": [100, 100],
            "brightness_range": [50, 100],
            "duration": 0,
            "transition": 0,
            "transition_range": [375, 500],
            "init_states": [[30, 81, 80]]
        });

        let effect: LightingEffectState =
            serde_json::from_value(json).expect("effect should parse");

        assert_eq!(effect.enable, 1);
        assert_eq!(effect.name, "Rainbow");
        assert_eq!(effect.custom, 0);
        assert_eq!(
            effect.id.as_deref(),
            Some("TapoStrip_4HVKmMc6vEzjm36jXaGwMs")
        );
        assert_eq!(effect.brightness, Some(100));
        assert_eq!(effect.segments.as_deref(), Some(&[1][..]));
        assert_eq!(effect.expansion_strategy, Some(1));
        assert_eq!(effect.effect_type.as_deref(), Some("random"));
        assert_eq!(effect.hue_range.as_deref(), Some(&[30, 40][..]));
        assert_eq!(effect.saturation_range.as_deref(), Some(&[100, 100][..]));
        assert_eq!(effect.brightness_range.as_deref(), Some(&[50, 100][..]));
        assert_eq!(effect.duration, Some(0));
        assert_eq!(effect.transition, Some(0));
        assert_eq!(effect.transition_range.as_deref(), Some(&[375, 500][..]));
        assert_eq!(effect.init_states.as_deref(), Some(&[vec![30, 81, 80]][..]));
    }

    #[test]
    fn lighting_effect_state_serializes_cleanly_without_empty_fields() {
        let effect = LightingEffectState {
            enable: 0,
            name: "Off".to_string(),
            custom: 0,
            id: None,
            brightness: None,
            segments: None,
            expansion_strategy: None,
            effect_type: None,
            hue_range: None,
            saturation_range: None,
            brightness_range: None,
            duration: None,
            transition: None,
            transition_range: None,
            init_states: None,
        };

        let value = serde_json::to_value(effect).expect("effect should serialize");
        assert_eq!(value["enable"], 0);
        assert_eq!(value["name"], "Off");
        assert_eq!(value["custom"], 0);
        assert!(value.get("id").is_none());
        assert!(value.get("segments").is_none());
        assert!(value.get("type").is_none());
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
