//! Bounded, read-only fleet checks. Duplicate addresses never cause duplicate logins.
use super::{DoctorReport, inspect_resolved};
use crate::{hosts, resolve::Resolved};
use anyhow::Result;
use futures_util::{StreamExt, stream};
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;

mod render;

pub(super) async fn bounded_inspect(resolved: Resolved, timeout: u16) -> DoctorReport {
    let mut fallback = DoctorReport::pending(
        resolved.saved_name.clone(),
        resolved.ip.clone(),
        &resolved.protocol,
    );
    match tokio::time::timeout(
        Duration::from_secs(u64::from(timeout)),
        inspect_resolved(resolved),
    )
    .await
    {
        Ok(Ok(report)) => report,
        result => {
            let error = match result {
                Ok(Err(error)) => error,
                Err(_) => crate::error::error(
                    "timeout",
                    "Device diagnostic deadline exceeded; no retry was attempted",
                ),
                Ok(Ok(_)) => unreachable!(),
            };
            fallback.record_failure(&error);
            fallback.add_guidance();
            fallback
        }
    }
}

fn group_addresses(
    entries: Vec<(String, hosts::HostEntry)>,
) -> BTreeMap<String, Vec<(String, hosts::HostEntry)>> {
    let mut groups = BTreeMap::<String, Vec<_>>::new();
    for (name, entry) in entries {
        let key = entry
            .ip
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| entry.ip.clone());
        groups.entry(key).or_default().push((name, entry));
    }
    groups
}

#[derive(Debug, Serialize)]
struct FleetAlias {
    alias: String,
    physically_confirmed: bool,
}

#[derive(Debug, Serialize)]
struct FleetDevice {
    aliases: Vec<FleetAlias>,
    diagnostics: DoctorReport,
}

#[derive(Debug, Serialize)]
struct FleetSummary {
    total: usize,
    ok: usize,
    failed: usize,
    with_warnings: usize,
}

#[derive(Debug, Serialize)]
struct FleetReport {
    devices: Vec<FleetDevice>,
    summary: FleetSummary,
}

impl FleetReport {
    fn new(mut devices: Vec<FleetDevice>) -> Self {
        devices.sort_by(|a, b| a.diagnostics.ip.cmp(&b.diagnostics.ip));
        let failed = devices
            .iter()
            .filter(|device| device.diagnostics.status != "ok")
            .count();
        let with_warnings = devices
            .iter()
            .filter(|device| !device.diagnostics.warnings.is_empty())
            .count();
        let total = devices.len();
        Self {
            devices,
            summary: FleetSummary {
                total,
                ok: total - failed,
                failed,
                with_warnings,
            },
        }
    }

    fn outcome(&self) -> Result<()> {
        if self.summary.failed > 0 {
            return Err(crate::error::error(
                "fleet_diagnostics_failed",
                "Some devices failed diagnostics; see individual results",
            ));
        }
        Ok(())
    }
}

/// Execute checks without reading configuration or emitting output.
/// The probe boundary lets offline tests exercise scheduling and conflict handling.
async fn inspect<F, Fut>(
    entries: Vec<(String, hosts::HostEntry)>,
    concurrency: u8,
    timeout: u16,
    probe: F,
) -> FleetReport
where
    F: Fn(Resolved, u16) -> Fut,
    Fut: std::future::Future<Output = DoctorReport>,
{
    let probe = &probe;
    let devices = stream::iter(group_addresses(entries).into_iter().map(|(ip, aliases)| async move {
        let (name, entry) = &aliases[0];
        let mut report = if aliases.iter().any(|(_, other)| other.protocol != entry.protocol || other.device_id != entry.device_id) {
            let mut report = DoctorReport::pending(Some(name.clone()), ip.clone(), &entry.protocol);
            report.record_failure(&crate::error::error("alias_conflict", "Aliases at this address disagree on protocol or identity; resolve them before checking the device"));
            report
        } else {
            probe(Resolved { ip, protocol: entry.protocol.clone(), saved_name: Some(name.clone()) }, timeout).await
        };
        report.physically_confirmed = aliases.iter().all(|(_, entry)| entry.physically_confirmed);
        FleetDevice {
            diagnostics: report,
            aliases: aliases.into_iter().map(|(alias, entry)| FleetAlias { alias, physically_confirmed: entry.physically_confirmed }).collect(),
        }
    })).buffer_unordered(usize::from(concurrency.max(1))).collect::<Vec<_>>().await;
    FleetReport::new(devices)
}

pub(super) async fn run(concurrency: u8, timeout: u16) -> Result<()> {
    let report = inspect(hosts::list()?, concurrency, timeout, bounded_inspect).await;
    render::emit(&report)?;
    report.outcome()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equivalent_addresses_are_probed_once_and_keep_aliases() {
        let entries = ["2001:db8::1", "2001:0db8:0:0:0:0:0:1"]
            .into_iter()
            .enumerate()
            .map(|(i, ip)| {
                (
                    format!("Alias {i}"),
                    serde_json::from_value(serde_json::json!({"ip":ip,"protocol":"kasa"})).unwrap(),
                )
            })
            .collect();
        let groups = group_addresses(entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups["2001:db8::1"].len(), 2);
    }
    #[tokio::test]
    async fn fleet_execution_bounds_probes_preserves_aliases_and_reports_conflicts() {
        use std::cell::{Cell, RefCell};
        let entries = || {
            vec![
                ("later", "192.0.2.3", true, "same"),
                ("first", "192.0.2.1", true, "same"),
                ("duplicate", "192.0.2.1", false, "same"),
                ("conflict-a", "192.0.2.2", false, "one"),
                ("conflict-b", "192.0.2.2", false, "two"),
            ]
            .into_iter()
            .map(|(name, ip, confirmed, id)| {
                (
            name.into(),
            serde_json::from_value(serde_json::json!({
                "ip": ip, "protocol": "kasa", "physically_confirmed": confirmed, "device_id": id
            })).unwrap()
        )
            })
            .collect()
        };

        for concurrency in [1, 2] {
            let active = Cell::new(0);
            let peak = Cell::new(0);
            let calls = RefCell::new(Vec::new());
            let report = inspect(entries(), concurrency, 7, |resolved, timeout| {
                let active = &active;
                let peak = &peak;
                let calls = &calls;
                async move {
                    assert_eq!(timeout, 7);
                    calls.borrow_mut().push(resolved.ip.clone());
                    active.set(active.get() + 1);
                    peak.set(peak.get().max(active.get()));
                    tokio::task::yield_now().await;
                    active.set(active.get() - 1);
                    let mut report =
                        DoctorReport::pending(resolved.saved_name, resolved.ip, &resolved.protocol);
                    report.status = "ok";
                    report.warnings.push("fixture warning".into());
                    report
                }
            })
            .await;
            assert_eq!(peak.get(), concurrency);
            assert_eq!(*calls.borrow(), ["192.0.2.1", "192.0.2.3"]);
            let value = serde_json::to_value(&report).unwrap();
            assert_eq!(
                value["summary"],
                serde_json::json!({"total":3,"ok":2,"failed":1,"with_warnings":2})
            );
            assert_eq!(
                value["devices"][0]["aliases"],
                serde_json::json!([
                    {"alias":"first","physically_confirmed":true},
                    {"alias":"duplicate","physically_confirmed":false}
                ])
            );
            assert_eq!(
                value["devices"][0]["diagnostics"]["physically_confirmed"],
                false
            );
            assert_eq!(value["devices"][1]["diagnostics"]["ip"], "192.0.2.2");
            assert_eq!(
                value["devices"][1]["diagnostics"]["error_code"],
                "alias_conflict"
            );
            assert_eq!(
                value["devices"][2]["diagnostics"]["physically_confirmed"],
                true
            );
            assert_eq!(
                crate::error::code(&report.outcome().unwrap_err()),
                "fleet_diagnostics_failed"
            );
        }
        let empty = inspect(Vec::new(), 1, 7, |_, _| async {
            panic!("empty fleet must not probe")
        })
        .await;
        assert!(empty.outcome().is_ok());
        assert_eq!(
            serde_json::to_value(empty).unwrap(),
            serde_json::json!({
                "devices": [], "summary": {"total":0,"ok":0,"failed":0,"with_warnings":0}
            })
        );
    }
}
