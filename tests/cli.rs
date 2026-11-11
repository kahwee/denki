//! CLI integration scenarios share one linked executable. No live devices.
#![cfg(target_os = "linux")]

#[path = "cli/automation_cli.rs"]
mod automation_cli;
#[path = "cli/process.rs"]
mod process;
#[path = "cli/protocol_resolution.rs"]
mod protocol_resolution;
#[path = "cli/scan_registry.rs"]
mod scan_registry;
#[path = "cli/storage_cli.rs"]
mod storage_cli;
#[path = "cli/tapo_aliases.rs"]
mod tapo_aliases;
#[path = "cli/terminal_cli.rs"]
mod terminal_cli;
