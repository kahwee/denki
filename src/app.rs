//! CLI application wiring and dispatch.

mod dispatch;
mod doctor;
mod info;
mod scan;
mod shared;

pub use dispatch::run;
