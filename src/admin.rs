#![allow(dead_code, unused_imports)]

mod auth;
mod completions;
mod host;
mod registry;

pub use auth::handle_login;
pub use completions::handle_completions;
pub use host::{
    handle_clock, handle_led, handle_outlet_rename, handle_outlets, handle_presets, handle_rename,
    handle_restart, handle_schedules, handle_specs,
};
pub use registry::{handle_alias, handle_aliases, handle_unalias};
