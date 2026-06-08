use super::support::{
    make_dimmer_for_hints, make_lightstrip_for_hints, make_plug_for_hints, make_strip_for_hints,
};
use crate::display::hints::{dimmer_hints, lightstrip_hints, plug_hints, strip_hints};

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
    let d = make_dimmer_for_hints();
    let h = dimmer_hints(&d, "d");
    assert!(h.iter().any(|s| s.contains("dim")), "hints: {h:?}");
    assert!(h.iter().any(|s| s.contains("schedules")), "hints: {h:?}");
}
