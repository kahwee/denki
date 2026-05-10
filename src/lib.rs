//! denki — TP-Link smart device library
//!
//! Core modules are public so downstream crates can drive Kasa and Tapo
//! devices without going through the CLI.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use denki::{klap, ops, transport};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Tapo device via KLAP
//!     let mut session = klap::handshake("192.168.7.254", "user@example.com", "pass").await?;
//!     ops::tapo_on(&mut session).await?;
//!
//!     // Legacy Kasa device via XOR
//!     ops::relay_on("192.168.4.23").await?;
//!     Ok(())
//! }
//! ```

pub mod bulb;
pub mod cipher;
pub mod creds;
pub mod devices;
pub mod dimmer;
pub mod fmt;
pub mod hosts;
pub mod klap;
pub mod ops;
pub mod plug;
pub mod strip;
pub mod tapo;
pub mod transport;

// display is CLI-only (uses `colored`); keep it out of the lib API
pub mod display;
