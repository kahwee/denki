//! Sysinfo for TP-Link smart plugs (KP115, HS110, HS105, etc.)
//!
//! Feature string: "TIM" = schedules, "ENE" = energy monitoring, "TIM:ENE" = both.
//! LED field is inverted: led_off=1 means the LED is dark, led_off=0 means it's lit.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Plug {
    pub alias: String,
    pub model: String,
    pub hw_ver: String,
    pub sw_ver: String,
    pub rssi: i32,
    pub relay_state: u8,
    #[serde(default)]
    pub on_time: u64,
    // Inverted: led_off=1 = LED dark, led_off=0 = LED lit
    #[serde(default)]
    pub led_off: u8,
    pub feature: Option<String>,
}

impl Plug {
    pub fn is_on(&self) -> bool {
        self.relay_state == 1
    }

    pub fn has_energy_monitoring(&self) -> bool {
        self.feature
            .as_deref()
            .is_some_and(|f| f.contains("ENE"))
    }

    pub fn on_time_fmt(&self) -> String {
        crate::fmt::on_time(self.on_time)
    }
}

pub fn parse(json: &serde_json::Value) -> Option<Plug> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    if !crate::devices::is_plug_switch(sysinfo) {
        return None;
    }
    serde_json::from_value(sysinfo.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

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

        #[test]
        fn returns_off_when_zero() {
            assert_eq!(make_plug(0, None).on_time_fmt(), "off");
        }

        #[test]
        fn delegates_to_fmt_duration_when_nonzero() {
            // Spot-check: the full range is tested in fmt::tests
            assert_eq!(make_plug(3661, None).on_time_fmt(), "1h 1m");
            assert_eq!(make_plug(90, None).on_time_fmt(), "1m 30s");
            assert_eq!(make_plug(45, None).on_time_fmt(), "45s");
        }
    }

    mod energy_monitoring {
        use super::*;

        #[rstest]
        #[case(Some("TIM:ENE"), true)]
        #[case(Some("ENE"), true)]
        #[case(Some("TIM"), false)]
        #[case(Some(""), false)]
        #[case(None, false)]
        fn detected_from_feature_string(#[case] feature: Option<&str>, #[case] expected: bool) {
            assert_eq!(
                make_plug(0, feature).has_energy_monitoring(),
                expected,
                "feature={feature:?}"
            );
        }
    }
}
