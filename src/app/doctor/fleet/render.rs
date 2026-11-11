//! Fleet output only: no configuration reads, scheduling, or device I/O.
use super::FleetReport;
use anyhow::Result;

pub(super) fn emit(report: &FleetReport) -> Result<()> {
    crate::output::record(serde_json::to_value(report)?);
    for device in &report.devices {
        super::super::print_text(&device.diagnostics);
        if device.aliases.len() > 1 {
            crate::output::println!(
                "  Shared address: {} saved aliases; checked once",
                device.aliases.len()
            );
        }
    }
    let summary = &report.summary;
    crate::output::println!(
        "Fleet: {} addresses, {} OK, {} failed, {} with warnings",
        summary.total,
        summary.ok,
        summary.failed,
        summary.with_warnings
    );
    Ok(())
}
