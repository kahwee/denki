use crate::bulb::{Bulb, LightState};
use crate::dimmer::Dimmer;
use crate::plug::Plug;
use crate::strip::Strip;
use serde_json::json;

pub(super) fn wday(bits: &[u8]) -> Vec<serde_json::Value> {
    bits.iter().map(|&b| json!(b)).collect()
}

pub(super) fn make_plug_for_hints(model: &str, is_on: bool, ene: bool) -> Plug {
    Plug {
        alias: "Test".to_string(),
        model: model.to_string(),
        hw_ver: "1.0".to_string(),
        sw_ver: "1.0.0".to_string(),
        rssi: -50,
        relay_state: u8::from(is_on),
        on_time: 0,
        led_off: 0,
        feature: if ene {
            Some("TIM:ENE".to_string())
        } else {
            Some("TIM".to_string())
        },
    }
}

pub(super) fn make_strip_for_hints(model: &str, ene: bool) -> Strip {
    Strip {
        alias: "Test".to_string(),
        model: model.to_string(),
        hw_ver: "1.0".to_string(),
        sw_ver: "1.0.0".to_string(),
        rssi: -40,
        relay_state: 0,
        feature: if ene {
            Some("TIM:ENE".to_string())
        } else {
            Some("TIM".to_string())
        },
        children: vec![],
    }
}

pub(super) fn make_lightstrip_for_hints(model: &str, is_on: bool) -> Bulb {
    Bulb {
        alias: "Test Light Strip".to_string(),
        model: model.to_string(),
        hw_ver: "1.0".to_string(),
        sw_ver: "1.0.0".to_string(),
        rssi: -40,
        is_color: 1,
        is_dimmable: 1,
        is_variable_color_temp: 1,
        light_state: LightState {
            on_off: u8::from(is_on),
            brightness: Some(80),
            color_temp: Some(0),
            hue: Some(5),
            saturation: Some(80),
            dft_on_state: None,
        },
        lighting_effect_state: None,
    }
}

pub(super) fn make_dimmer_for_hints() -> Dimmer {
    Dimmer {
        alias: "d".to_string(),
        model: "HS220".to_string(),
        hw_ver: "1.0".to_string(),
        sw_ver: "1.0.0".to_string(),
        rssi: -50,
        relay_state: 0,
        brightness: 80,
        feature: None,
    }
}
