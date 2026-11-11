//! Fleet output only: no configuration reads, scheduling, or device I/O.
use super::FleetReport;

pub(super) fn emit(report: &FleetReport, failed_only: bool) {
    let devices: Vec<_> = report
        .devices
        .iter()
        .filter(|device| !failed_only || device.diagnostics.status != "ok")
        .collect();
    crate::output::record(serde_json::json!({"devices": devices, "summary": report.summary}));
    for device in devices {
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
}
