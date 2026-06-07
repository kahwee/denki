use super::*;
use rstest::rstest;
use serde_json::json;

fn wday(bits: &[u8]) -> Vec<serde_json::Value> {
    bits.iter().map(|&b| json!(b)).collect()
}

#[rstest]
#[case(None, "every day")]
#[case(Some(&[1u8,0,0,0,0,0,0][..]), "Sun")]
#[case(Some(&[0,1,0,0,0,0,0][..]), "Mon")]
#[case(Some(&[0,0,0,0,0,1,0][..]), "Fri")]
#[case(Some(&[0,1,0,1,0,1,0][..]), "Mon Wed Fri")]
#[case(Some(&[1,1,1,1,1,1,1][..]), "every day")]
#[case(Some(&[0,0,0,0,0,0,0][..]), "no days")]
fn format_wday_cases(#[case] bits: Option<&[u8]>, #[case] expected: &str) {
    let v = bits.map(wday);
    assert_eq!(format_wday(v.as_ref()), expected);
}

#[test]
fn energy_lines_kp115_all_fields() {
    let d = json!({
        "power_mw": 5400.0, "voltage_mv": 120100.0,
        "current_ma": 45.0, "total_wh": 12345
    });
    insta::assert_snapshot!(format_energy_lines(&d).join("\n"));
}

#[test]
fn energy_lines_hs110_all_fields() {
    let d = json!({
        "power": 5.4, "voltage": 120.1, "current": 0.045, "total": 12.345
    });
    insta::assert_snapshot!(format_energy_lines(&d).join("\n"));
}

#[test]
fn energy_lines_kl135_power_and_total_only() {
    let d = json!({"power_mw": 9000.0, "total_wh": 500});
    insta::assert_snapshot!(format_energy_lines(&d).join("\n"));
}

#[rstest]
#[case("1.1.1 Build 250908 Rel.112945", "1.1.1")]
#[case("1.0.15 Build 240429 Rel.154143", "1.0.15")]
#[case("1.0.9 Build 250627 Rel.180045", "1.0.9")]
#[case("1.0.9", "1.0.9")]
#[case("", "")]
fn short_fw_strips_build_suffix(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(short_fw(input), expected);
}

#[rstest]
#[case(  0, 100, 100, (255,   0,   0))]
#[case( 60, 100, 100, (255, 255,   0))]
#[case(120, 100, 100, (  0, 255,   0))]
#[case(180, 100, 100, (  0, 255, 255))]
#[case(240, 100, 100, (  0,   0, 255))]
#[case(300, 100, 100, (255,   0, 255))]
#[case(  0,   0, 100, (255, 255, 255))]
#[case(  0,   0,   0, (  0,   0,   0))]
fn hsv_to_rgb_primary_colors(
    #[case] h: u16,
    #[case] s: u8,
    #[case] v: u8,
    #[case] expected: (u8, u8, u8),
) {
    assert_eq!(hsv_to_rgb(h, s, v), expected, "hsv({h},{s},{v})");
}

#[test]
fn wh_from_integer_energy_wh() {
    let entry = serde_json::json!({"energy_wh": 1500});
    assert_eq!(wh_from(&entry), 1500);
}

#[test]
fn wh_from_rounds_kwh_not_truncates() {
    let entry = serde_json::json!({"energy": 1.9999});
    assert_eq!(wh_from(&entry), 2000);
}

#[test]
fn wh_from_prefers_energy_wh_over_energy() {
    let entry = serde_json::json!({"energy_wh": 500, "energy": 1.0});
    assert_eq!(wh_from(&entry), 500);
}

#[test]
fn wh_from_returns_zero_when_no_energy_fields() {
    assert_eq!(wh_from(&serde_json::json!({})), 0);
    assert_eq!(wh_from(&serde_json::json!({"day": 1})), 0);
}

#[test]
fn hsv_to_rgb_kl135_purple_hue_308() {
    let (r, g, b) = hsv_to_rgb(308, 65, 100);
    assert!(r > g, "expected red > green for purple: ({r},{g},{b})");
    assert!(b > g, "expected blue > green for purple: ({r},{g},{b})");
}

#[test]
fn sort_energy_entries_orders_ascending_by_key() {
    let days = vec![
        json!({"day": 3, "energy_wh": 300}),
        json!({"day": 1, "energy_wh": 100}),
        json!({"day": 2, "energy_wh": 200}),
    ];
    let sorted = sort_energy_entries(&days, "day");
    assert_eq!(
        sorted.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(sorted[0].1, 100);

    let months = vec![
        json!({"month": 12, "energy_wh": 1200}),
        json!({"month":  3, "energy_wh":  300}),
        json!({"month":  7, "energy_wh":  700}),
    ];
    let sorted = sort_energy_entries(&months, "month");
    assert_eq!(
        sorted.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        [3, 7, 12]
    );
    assert_eq!(sorted[0].1, 300);
    assert_eq!(sorted[2].1, 1200);
}

#[test]
fn sort_energy_entries_empty_list() {
    assert!(sort_energy_entries(&[], "day").is_empty());
}

fn make_plug_for_hints(model: &str, is_on: bool, ene: bool) -> crate::plug::Plug {
    crate::plug::Plug {
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

fn make_strip_for_hints(model: &str, ene: bool) -> crate::strip::Strip {
    crate::strip::Strip {
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

fn make_lightstrip_for_hints(model: &str, is_on: bool) -> crate::bulb::Bulb {
    crate::bulb::Bulb {
        alias: "Test Light Strip".to_string(),
        model: model.to_string(),
        hw_ver: "1.0".to_string(),
        sw_ver: "1.0.0".to_string(),
        rssi: -40,
        is_color: 1,
        is_dimmable: 1,
        is_variable_color_temp: 1,
        light_state: crate::bulb::LightState {
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

#[test]
fn plug_hints_ene_plug_includes_energy() {
    let p = make_plug_for_hints("KP115", false, true);
    let h = plug_hints(&p, "plug");
    assert!(h.iter().any(|s| s.contains("energy")), "hints: {h:?}");
}

#[test]
fn plug_hints_no_ene_excludes_energy() {
    let p = make_plug_for_hints("HS105", false, false);
    let h = plug_hints(&p, "plug");
    assert!(!h.iter().any(|s| s.contains("energy")), "hints: {h:?}");
}

#[test]
fn plug_hints_on_plug_starts_with_off() {
    let p = make_plug_for_hints("KP115", true, true);
    assert_eq!(plug_hints(&p, "p")[0], "denki off \"p\"");
}

#[test]
fn plug_hints_off_plug_starts_with_on() {
    let p = make_plug_for_hints("KP115", false, true);
    assert_eq!(plug_hints(&p, "p")[0], "denki on \"p\"");
}

#[test]
fn strip_hints_ene_strip_includes_per_outlet_energy() {
    let s = make_strip_for_hints("HS300", true);
    let h = strip_hints(&s, "strip");
    assert!(
        h.iter().any(|s| s.contains("energy") && s.contains(" 1")),
        "hints: {h:?}"
    );
}

#[test]
fn strip_hints_no_ene_excludes_energy() {
    let s = make_strip_for_hints("KP303", false);
    let h = strip_hints(&s, "strip");
    assert!(!h.iter().any(|s| s.contains("energy")), "hints: {h:?}");
}

#[test]
fn strip_hints_always_includes_per_outlet_on_off() {
    let s = make_strip_for_hints("HS300", true);
    let h = strip_hints(&s, "strip");
    assert!(h.iter().any(|s| s.contains("on") && s.contains(" 1")));
    assert!(h.iter().any(|s| s.contains("off") && s.contains(" 1")));
}

#[test]
fn strip_hints_includes_outlet_rename() {
    let s = make_strip_for_hints("HS300", true);
    let h = strip_hints(&s, "strip");
    assert!(
        h.iter().any(|s| s.contains("outlet-rename")),
        "hints: {h:?}"
    );
}

#[test]
fn lightstrip_hints_for_kl420l5_include_energy_and_monthly_commands() {
    let b = make_lightstrip_for_hints("KL420L5", false);
    let h = lightstrip_hints(&b, "lights");
    assert!(
        h.iter().any(|s| s.contains("energy \"lights\"")),
        "hints: {h:?}"
    );
    assert!(
        h.iter().any(|s| s.contains("energy-daily \"lights\"")),
        "hints: {h:?}"
    );
    assert!(
        h.iter().any(|s| s.contains("energy-monthly \"lights\"")),
        "hints: {h:?}"
    );
    assert!(
        !h.iter().any(|s| s == "denki on \"lights\""),
        "light strip should not advertise power control: {h:?}"
    );
}

#[test]
fn dimmer_hints_includes_dim_and_schedules() {
    use crate::dimmer::Dimmer;
    let d = Dimmer {
        alias: "d".to_string(),
        model: "HS220".to_string(),
        hw_ver: "1.0".to_string(),
        sw_ver: "1.0.0".to_string(),
        rssi: -50,
        relay_state: 0,
        brightness: 80,
        feature: None,
    };
    let h = dimmer_hints(&d, "d");
    assert!(h.iter().any(|s| s.contains("dim")), "hints: {h:?}");
    assert!(h.iter().any(|s| s.contains("schedules")), "hints: {h:?}");
}
