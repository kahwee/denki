use super::registry::DeviceKind;
use anyhow::Result;

// Command capability guards — pure functions, called before any network I/O.

pub fn can_control_power(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        DeviceKind::LightStrip => anyhow::bail!(
            "light strip power control is not yet implemented \
             (KL430 uses smartlife.iot.lightStrip)"
        ),
        other => anyhow::bail!("{other} does not support power control"),
    }
}

pub fn can_dim(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::Dimmer => Ok(()),
        DeviceKind::LightStrip => anyhow::bail!(
            "`dim` is not yet supported on light strips \
             (KL430 uses smartlife.iot.lightStrip, not smartbulb.lightingservice)"
        ),
        other => anyhow::bail!("`dim` is only supported on bulbs and HS220 dimmers, not {other}"),
    }
}

fn require_bulb(kind: &DeviceKind, cmd: &str, models: &str) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!("`{cmd}` is only supported on {models}, not {other}"),
    }
}

fn require_relay_device(kind: &DeviceKind, cmd: &str) -> Result<()> {
    match kind {
        DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        other => anyhow::bail!(
            "`{cmd}` is only supported on plugs, dimmers, and strips \
             (e.g. KP115, HS220, HS300), not {other}"
        ),
    }
}

pub fn can_set_color_temp(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "color-temp", "color bulbs (e.g. KL135)")
}

pub fn can_set_color(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "color", "bulbs")
}

pub fn can_get_specs(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "specs", "bulbs")
}

pub fn can_get_presets(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "presets", "bulbs")
}

pub fn can_get_effects(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::LightStrip => Ok(()),
        other => anyhow::bail!("`effects` is only supported on light strips, not {other}"),
    }
}

pub fn can_get_schedules(kind: &DeviceKind) -> Result<()> {
    require_relay_device(kind, "schedules")
}

pub fn can_control_led(kind: &DeviceKind) -> Result<()> {
    require_relay_device(kind, "led")
}

pub fn can_get_clock(kind: &DeviceKind) -> Result<()> {
    require_relay_device(kind, "clock")
}
