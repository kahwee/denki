//! Device capability registry and capability checks.

mod classify;
mod energy;
mod guards;
mod registry;

pub use classify::{detect_kind, is_plug_switch};
pub use energy::require_energy;
pub use guards::{
    can_control_led, can_control_power, can_dim, can_get_clock, can_get_effects, can_get_presets,
    can_get_schedules, can_get_specs, can_set_color, can_set_color_temp,
};
pub use registry::{DeviceEntry, DeviceKind, all, hint_for, hints, lookup};

#[cfg(test)]
mod tests;
