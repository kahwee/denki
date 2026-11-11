//! Bounded, read-only fleet checks. Duplicate addresses never cause duplicate logins.
use super::{DoctorReport, inspect_resolved, print_text};
use crate::{hosts, resolve::Resolved};
use anyhow::Result;
use futures_util::{StreamExt, stream};
use std::collections::BTreeMap;
use std::time::Duration;

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

pub(super) async fn run(concurrency: u8, timeout: u16) -> Result<()> {
    let groups = group_addresses(hosts::list()?);
    let mut results = stream::iter(groups.into_iter().map(|(ip, aliases)| async move {
        let (name, entry) = &aliases[0];
        let mut report = if aliases.iter().any(|(_, other)| other.protocol != entry.protocol || other.device_id != entry.device_id) {
            let mut report = DoctorReport::pending(Some(name.clone()), ip.clone(), &entry.protocol);
            report.record_failure(&crate::error::error("alias_conflict", "Aliases at this address disagree on protocol or identity; resolve them before checking the device"));
            report
        } else {
            bounded_inspect(Resolved { ip: ip.clone(), protocol: entry.protocol.clone(), saved_name: Some(name.clone()) }, timeout).await
        };
        report.physically_confirmed = aliases.iter().all(|(_, entry)| entry.physically_confirmed);
        (ip, report, aliases.into_iter().map(|(name, entry)| serde_json::json!({"alias":name,"physically_confirmed":entry.physically_confirmed})).collect::<Vec<_>>())
    })).buffer_unordered(usize::from(concurrency)).collect::<Vec<_>>().await;
    results.sort_by(|a, b| a.0.cmp(&b.0));
    let failed = results
        .iter()
        .filter(|(_, report, _)| report.status != "ok")
        .count();
    let warnings = results
        .iter()
        .filter(|(_, report, _)| !report.warnings.is_empty())
        .count();
    let devices = results
        .iter()
        .map(|(_, report, aliases)| serde_json::json!({"aliases":aliases,"diagnostics":report}))
        .collect::<Vec<_>>();
    crate::output::record(
        serde_json::json!({"devices":devices,"summary":{"total":results.len(),"ok":results.len()-failed,"failed":failed,"with_warnings":warnings}}),
    );
    for (_, report, aliases) in &results {
        print_text(report);
        if aliases.len() > 1 {
            crate::output::println!(
                "  Shared address: {} saved aliases; checked once",
                aliases.len()
            );
        }
    }
    crate::output::println!(
        "Fleet: {} addresses, {} OK, {} failed, {} with warnings",
        results.len(),
        results.len() - failed,
        failed,
        warnings
    );
    if failed > 0 {
        return Err(crate::error::error(
            "fleet_diagnostics_failed",
            "Some devices failed diagnostics; see individual results",
        ));
    }
    Ok(())
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
}
