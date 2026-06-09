use anyhow::Result;
use colored::Colorize;

use crate::hosts;
use crate::ops;
use crate::tapo;
use crate::transport;

use super::shared::{print_kasa_summary, tapo_session};

pub(super) async fn handle_scan(timeout: u64) -> Result<()> {
    println!("{}", format!("Scanning network for {timeout}s...").dimmed());
    let mut host_map = hosts::load().unwrap_or_default();
    let mut map_dirty = false;
    let mut device_count = transport::broadcast_each(timeout, |ip, json| {
        let ip_str = ip.to_string();
        let is_new = json
            .pointer("/system/get_sysinfo/alias")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| hosts::save_if_new_in(name, &ip_str, &mut host_map));
        if is_new {
            map_dirty = true;
        }
        let hint = hosts::lookup_by_ip_in(&ip_str, &host_map).unwrap_or_else(|| ip_str.clone());
        print_kasa_summary(ip, &json, &hint);
        if is_new {
            println!("{}", "  ↳ (new) alias auto-saved".dimmed());
        }
    })
    .await?;
    if map_dirty {
        hosts::save(&host_map)?;
    }

    let klap_aliases: Vec<(String, hosts::HostEntry)> = host_map
        .into_iter()
        .filter(|(_, v)| v.protocol == hosts::Protocol::Klap)
        .collect();
    let mut join_set = tokio::task::JoinSet::new();
    for (name, entry) in klap_aliases {
        join_set.spawn(async move {
            let ip = entry.ip;
            let mut session = tapo_session(&ip).await.ok()?;
            let json = ops::tapo_device_info(&mut session).await.ok()?;
            let d = tapo::parse(&json)?;
            Some((ip, name, d))
        });
    }
    while let Some(result) = join_set.join_next().await {
        if let Ok(Some((ip, name, d))) = result {
            crate::display::print_tapo_summary(&ip, &d, &name);
            device_count += 1;
        }
    }

    if device_count == 0 {
        println!("No devices found.");
    } else {
        println!("{}", format!("Found {device_count} device(s)").dimmed());
    }
    Ok(())
}
