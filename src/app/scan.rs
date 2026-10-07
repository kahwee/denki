use super::shared::{print_kasa_summary, tapo_session};
use crate::{hosts, ops, tapo, transport};
use anyhow::Result;
use futures_util::{StreamExt, stream};
use serde_json::json;
use std::net::IpAddr;

fn probe_targets(
    map: &std::collections::BTreeMap<String, hosts::HostEntry>,
    additional: Vec<IpAddr>,
) -> Result<Vec<(String, hosts::Protocol)>> {
    let mut addresses = std::collections::BTreeSet::new();
    for entry in map
        .values()
        .filter(|entry| entry.protocol != hosts::Protocol::Kasa)
    {
        addresses.insert(entry.ip.parse::<IpAddr>()?);
    }
    addresses.extend(additional);
    addresses
        .into_iter()
        .map(|address| {
            let ip = address.to_string();
            let protocol = hosts::protocol_by_ip_in(&ip, map)?.unwrap_or(hosts::Protocol::Tapo);
            if protocol == hosts::Protocol::Kasa {
                return Err(crate::error::error(
                    "invalid_arguments",
                    "A --tapo-target address is saved as Kasa; update its alias protocol first",
                ));
            }
            Ok((ip, protocol))
        })
        .collect()
}

pub(super) async fn handle_scan(timeout: u64, tapo_targets: Vec<IpAddr>) -> Result<()> {
    let mut map = hosts::load()?;
    let previous = map.clone();
    // Resolve conflicts before any discovery traffic or registry changes.
    let targets = probe_targets(&map, tapo_targets)?;
    crate::output::println!("Scanning network for {timeout}s...");
    let mut found = Vec::new();
    transport::broadcast_each(timeout, |ip, response| found.push((ip, response))).await?;
    // Reconcile known identities before learning new devices, including swapped leases.
    found.sort_by_key(|(_, response)| {
        let id = response
            .pointer("/system/get_sysinfo/deviceId")
            .and_then(|v| v.as_str());
        !map.values().any(|entry| {
            id.is_some()
                && entry.device_id.as_deref() == id
                && entry.protocol == hosts::Protocol::Kasa
        })
    });
    let mut dirty = false;
    let mut results = Vec::new();
    for (ip, response) in found {
        let outcome = ops::validate_sysinfo(&response).and_then(|()| {
            let info = &response["system"]["get_sysinfo"];
            hosts::reconcile_in(
                info["alias"].as_str().unwrap_or(""),
                &ip.to_string(),
                hosts::Protocol::Kasa,
                info["deviceId"].as_str(),
                &mut map,
            )
        });
        match outcome {
            Ok(changed) => {
                dirty |= changed;
                let alias = hosts::lookup_by_ip_in(&ip.to_string(), &map);
                print_kasa_summary(ip, &response, alias.as_deref().unwrap_or(""));
                results.push(json!({"ip":ip.to_string(),"protocol":"kasa","alias":alias,"status":"ok","registry_updated":changed,
                    "device":crate::output::sanitized(response["system"]["get_sysinfo"].clone())}));
            }
            Err(error) => {
                eprintln!("{ip}: {error}");
                results.push(json!({"ip":ip.to_string(),"protocol":"kasa","status":"error","error":crate::output::failure(&error)}));
            }
        }
    }
    let probes = stream::iter(targets.into_iter().map(|(ip, protocol)| async move {
        let result = async {
            if protocol == hosts::Protocol::Tapo {
                return crate::tapo_client::probe_info(&ip).await;
            }
            let mut session = tapo_session(&ip).await?;
            let response = ops::tapo_probe_info(&mut session).await?;
            tapo::parse(&response)
                .ok_or_else(|| crate::error::malformed("Could not parse Tapo device info"))
        }
        .await;
        (ip, protocol, result)
    }))
    .buffer_unordered(4)
    .collect::<Vec<_>>()
    .await;
    for (ip, protocol, result) in probes {
        let result = result.and_then(|device| {
            let changed = hosts::reconcile_in(
                &device.nickname,
                &ip,
                protocol.clone(),
                Some(&device.device_id),
                &mut map,
            )?;
            dirty |= changed;
            Ok(device)
        });
        match result {
            Ok(device) => {
                let alias = hosts::lookup_by_ip_in(&ip, &map);
                crate::display::print_tapo_summary(&ip, &device, alias.as_deref().unwrap_or(""));
                results.push(json!({"ip":ip,"protocol":protocol,"alias":alias,"status":"ok","device":crate::output::sanitized(serde_json::to_value(device)?)}));
            }
            Err(error) => {
                eprintln!("{ip}: {error}");
                results.push(json!({"ip":ip,"protocol":protocol,"status":"error","error":crate::output::failure(&error)}));
            }
        }
    }
    if dirty {
        hosts::save_if_unchanged(&previous, &map)?;
    }
    results.sort_by(|a, b| a["ip"].as_str().cmp(&b["ip"].as_str()));
    let failed = results.iter().filter(|r| r["status"] == "error").count();
    crate::output::println!(
        "Found {} device(s); {failed} failed probes.",
        results.len() - failed
    );
    crate::output::record(json!({"devices":results,"failed":failed,"registry_updated":dirty}));
    if failed > 0 {
        return Err(crate::error::error(
            "partial_failure",
            format!("{failed} discovery probes failed"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn entry(ip: &str, protocol: hosts::Protocol) -> hosts::HostEntry {
        hosts::HostEntry {
            ip: ip.into(),
            protocol,
            device_id: None,
        }
    }

    #[test]
    fn targets_preserve_saved_protocol_and_negotiate_new_addresses_once() {
        let map = BTreeMap::from([
            ("legacy".into(), entry("192.0.2.1", hosts::Protocol::Klap)),
            ("auto".into(), entry("192.0.2.2", hosts::Protocol::Tapo)),
            (
                "duplicate".into(),
                entry("192.0.2.2", hosts::Protocol::Tapo),
            ),
        ]);
        let additional = ["192.0.2.1", "192.0.2.2", "192.0.2.3", "192.0.2.3"]
            .map(|ip| ip.parse().unwrap())
            .to_vec();
        assert_eq!(
            probe_targets(&map, additional).unwrap(),
            vec![
                ("192.0.2.1".into(), hosts::Protocol::Klap),
                ("192.0.2.2".into(), hosts::Protocol::Tapo),
                ("192.0.2.3".into(), hosts::Protocol::Tapo),
            ]
        );
    }

    #[test]
    fn conflicting_aliases_and_kasa_targets_are_rejected() {
        let mut map = BTreeMap::from([
            ("legacy".into(), entry("192.0.2.1", hosts::Protocol::Klap)),
            ("auto".into(), entry("192.0.2.1", hosts::Protocol::Tapo)),
        ]);
        assert_eq!(
            crate::error::code(&probe_targets(&map, vec![]).unwrap_err()),
            "ambiguous_protocol"
        );
        map.clear();
        map.insert("kasa".into(), entry("192.0.2.1", hosts::Protocol::Kasa));
        assert_eq!(
            crate::error::code(
                &probe_targets(&map, vec!["192.0.2.1".parse().unwrap()]).unwrap_err()
            ),
            "invalid_arguments"
        );
    }
}
