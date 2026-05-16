//! denki — TP-Link Kasa and Tapo device library.
//!
//! ```rust,no_run
//! use denki::{klap, ops};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Tapo device via KLAP
//!     let mut session = klap::handshake("192.168.7.254", "user@example.com", "pass").await?;
//!     ops::tapo_on(&mut session).await?;
//!     // Legacy Kasa device via XOR
//!     ops::relay_on("192.168.4.23").await?;
//!     Ok(())
//! }
//! ```

pub mod bulb;
pub mod cipher;
pub mod cli;
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

// display is CLI-focused (uses `colored`); keep it public for the binary to reuse.
pub mod display;
