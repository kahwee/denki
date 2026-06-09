use crate::devices::{self, DeviceKind};

fn guard_kind(kind: &DeviceKind) -> Option<&DeviceKind> {
    match kind {
        DeviceKind::Tapo => None,
        k => Some(k),
    }
}

const GUARDED: &[&str] = &[
    "power",
    "dim",
    "color_temp",
    "color",
    "specs",
    "presets",
    "schedules",
    "led",
    "clock",
];

fn check(kind: &DeviceKind, feature: &str) -> anyhow::Result<()> {
    match feature {
        "power" => devices::can_control_power(kind),
        "dim" => devices::can_dim(kind),
        "color_temp" => devices::can_set_color_temp(kind),
        "color" => devices::can_set_color(kind),
        "specs" => devices::can_get_specs(kind),
        "presets" => devices::can_get_presets(kind),
        "schedules" => devices::can_get_schedules(kind),
        "led" => devices::can_control_led(kind),
        "clock" => devices::can_get_clock(kind),
        "energy" | "outlets" | "effects" => Ok(()),
        other => panic!(
            "devices.toml: unknown feature '{other}' — add it to check() or explain \
             why it has no guard"
        ),
    }
}

#[test]
fn listed_features_are_permitted_by_guards() {
    for dev in devices::all() {
        let Some(kind) = guard_kind(&dev.kind) else {
            continue;
        };
        for feature in &dev.supports {
            let result = check(kind, feature);
            assert!(
                result.is_ok(),
                "devices.toml: {} ({}) lists '{}' but the guard rejects it: {}",
                dev.model,
                dev.kind,
                feature,
                result.unwrap_err(),
            );
        }
    }
}

#[test]
fn unlisted_guarded_features_are_denied() {
    for dev in devices::all() {
        let Some(kind) = guard_kind(&dev.kind) else {
            continue;
        };
        for &feature in GUARDED {
            if dev.supports.iter().any(|f| f == feature) {
                continue;
            }
            let result = check(kind, feature);
            assert!(
                result.is_err(),
                "devices.toml: {} ({}) does NOT list '{}' but the guard permits it — \
                 add it to 'supports' or tighten the guard",
                dev.model,
                dev.kind,
                feature,
            );
        }
    }
}
