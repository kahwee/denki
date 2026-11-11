//! Fleet output only: no configuration reads, scheduling, or device I/O.
use super::{FleetReport, Progress};
use crate::output::terminal_text;

pub(super) fn emit(report: &FleetReport, failed_only: bool) {
    let devices: Vec<_> = report
        .devices
        .iter()
        .filter(|device| !failed_only || device.diagnostics.status != "ok")
        .collect();
    crate::output::record(serde_json::json!({"devices": devices, "summary": report.summary}));
    for device in devices {
        super::super::print_text(&device.diagnostics);
        crate::output::println!("  Elapsed:      {} ms", device.elapsed_ms);
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

// Progress always goes to stderr, including when stdout contains JSON.
pub(super) fn progress(event: Progress<'_>) {
    match event {
        Progress::Started { total } => eprintln!("Checking {total} saved addresses..."),
        Progress::Completed {
            completed,
            total,
            device,
        } => eprintln!(
            "Checked {completed}/{total}: {} — {} ({} ms)",
            terminal_text(&device.diagnostics.ip),
            device.diagnostics.status,
            device.elapsed_ms
        ),
    }
}
