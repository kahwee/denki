use super::shared::{print_kasa_summary, tapo_session};
use crate::{hosts, ops, tapo, transport};
use anyhow::Result;
use futures_util::{StreamExt, stream};
use serde_json::json;
use std::net::IpAddr;

pub(super) async fn handle_scan(timeout: u64, tapo_targets: Vec<IpAddr>) -> Result<()> {
    let mut map = hosts::load()?;
    let previous = map.clone();
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
    let mut targets: Vec<(String, hosts::Protocol)> = map
        .values()
        .filter(|e| e.protocol != hosts::Protocol::Kasa)
        .map(|e| (e.ip.clone(), e.protocol.clone()))
        .collect();
    targets.extend(
        tapo_targets
            .into_iter()
            .map(|ip| (ip.to_string(), hosts::Protocol::Klap)),
    );
    targets.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
    });
    targets.dedup();
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
