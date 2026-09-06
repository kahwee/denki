use crate::devices::{self, DeviceKind};
use crate::hosts;
use crate::ops;
use crate::resolve::{Resolved, resolve};
use crate::strip;
use anyhow::Result;
use clap::ValueEnum;
use colored::Colorize;
use futures_util::{StreamExt, stream};

use super::shared::{
    StripOutletTarget, print_outlet_power_state, print_outlet_toggle_state, print_power_state,
    resolve_power_target, tapo_session,
};

fn toggle_target(kind: &DeviceKind, json: &serde_json::Value) -> bool {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => {
            json.pointer("/system/get_sysinfo/light_state/on_off")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
        }
        DeviceKind::Strip => !strip::parse(json).is_some_and(|s| s.is_any_on()),
        _ => {
            json.pointer("/system/get_sysinfo/relay_state")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
        }
    }
}

async fn kasa_set_power(ip: &str, kind: &DeviceKind, on: bool) -> Result<()> {
    devices::can_control_power(kind)?;
    match (kind, on) {
        (DeviceKind::Bulb, true) => ops::bulb_on(ip).await?,
        (DeviceKind::Bulb, false) => ops::bulb_off(ip).await?,
        (DeviceKind::LightStrip, state) => ops::lightstrip_set_power(ip, state).await?,
        (_, true) => ops::relay_on(ip).await?,
        (_, false) => ops::relay_off(ip).await?,
    }
    Ok(())
}

async fn set_device_power(r: &Resolved, on: bool) -> Result<()> {
    match r.protocol {
        hosts::Protocol::Klap => {
            let mut s = tapo_session(&r.ip).await?;
            if on {
                ops::tapo_on(&mut s).await?;
            } else {
                ops::tapo_off(&mut s).await?;
            }
        }
        hosts::Protocol::Kasa => {
            let json = ops::sysinfo(&r.ip).await?;
            kasa_set_power(&r.ip, &devices::detect_kind(&json), on).await?;
        }
    }
    Ok(())
}

trait DeviceTransport: Sync {
    async fn apply_power(&self, target: &Resolved, action: GroupAction) -> Result<bool>;
}

struct LiveDeviceTransport;

impl DeviceTransport for LiveDeviceTransport {
    async fn apply_power(&self, target: &Resolved, action: GroupAction) -> Result<bool> {
        match action {
            GroupAction::On => {
                set_device_power(target, true).await?;
                Ok(true)
            }
            GroupAction::Off => {
                set_device_power(target, false).await?;
                Ok(false)
            }
            GroupAction::Toggle => match target.protocol {
                hosts::Protocol::Klap => {
                    let mut session = tapo_session(&target.ip).await?;
                    ops::tapo_toggle(&mut session).await
                }
                hosts::Protocol::Kasa => {
                    let json = ops::sysinfo(&target.ip).await?;
                    let kind = devices::detect_kind(&json);
                    let on = toggle_target(&kind, &json);
                    kasa_set_power(&target.ip, &kind, on).await?;
                    Ok(on)
                }
            },
        }
    }
}

pub async fn handle_on(host: &str, outlet: Option<u8>) -> Result<()> {
    let (r, target) = resolve_power_target(host, outlet, "on <outlet>").await?;
    if let Some(outlet_num) = outlet {
        let StripOutletTarget {
            child_id,
            child_alias,
            ..
        } = target.expect("outlet target should be present here");
        ops::strip_outlet_on(&r.ip, &child_id).await?;
        print_outlet_power_state(outlet_num, &child_alias, true);
    } else {
        set_device_power(&r, true).await?;
        print_power_state(&r.ip, true);
    }
    Ok(())
}

pub async fn handle_off(host: &str, outlet: Option<u8>) -> Result<()> {
    let (r, target) = resolve_power_target(host, outlet, "off <outlet>").await?;
    if let Some(outlet_num) = outlet {
        let StripOutletTarget {
            child_id,
            child_alias,
            ..
        } = target.expect("outlet target should be present here");
        ops::strip_outlet_off(&r.ip, &child_id).await?;
        print_outlet_power_state(outlet_num, &child_alias, false);
    } else {
        set_device_power(&r, false).await?;
        print_power_state(&r.ip, false);
    }
    Ok(())
}

pub async fn handle_toggle(host: &str, outlet: Option<u8>) -> Result<()> {
    let (r, target) = resolve_power_target(host, outlet, "toggle <outlet>").await?;
    if let Some(outlet_num) = outlet {
        let StripOutletTarget {
            child_id,
            child_alias,
            was_on,
        } = target.expect("outlet target should be present here");
        let now_on = if was_on {
            ops::strip_outlet_off(&r.ip, &child_id).await?;
            false
        } else {
            ops::strip_outlet_on(&r.ip, &child_id).await?;
            true
        };
        print_outlet_toggle_state(outlet_num, &child_alias, now_on);
    } else {
        let now_on = match r.protocol {
            hosts::Protocol::Klap => {
                let mut s = tapo_session(&r.ip).await?;
                ops::tapo_toggle(&mut s).await?
            }
            hosts::Protocol::Kasa => {
                let json = ops::sysinfo(&r.ip).await?;
                let kind = devices::detect_kind(&json);
                let on = toggle_target(&kind, &json);
                kasa_set_power(&r.ip, &kind, on).await?;
                on
            }
        };
        print_power_state(&r.ip, now_on);
    }
    Ok(())
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum GroupAction {
    /// Turn every matched alias on.
    On,
    /// Turn every matched alias off.
    Off,
    /// Toggle every matched alias.
    Toggle,
}

impl GroupAction {
    fn as_verb(&self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Toggle => "toggle",
        }
    }
}

#[derive(Debug)]
struct GroupResult {
    alias: String,
    result: Result<bool>,
}

async fn execute_group<T: DeviceTransport>(
    matches: Vec<(String, hosts::HostEntry)>,
    action: GroupAction,
    concurrency: usize,
    transport: &T,
) -> Vec<GroupResult> {
    stream::iter(matches.into_iter().map(|(alias, entry)| async move {
        let target = Resolved {
            ip: entry.ip,
            protocol: entry.protocol,
            saved_name: Some(alias.clone()),
        };
        GroupResult {
            alias,
            result: transport.apply_power(&target, action).await,
        }
    }))
    .buffer_unordered(concurrency)
    .collect()
    .await
}

pub async fn handle_group(
    pattern: &str,
    action: GroupAction,
    dry_run: bool,
    concurrency: usize,
) -> Result<()> {
    let matches = hosts::lookup_many(pattern)?;
    if matches.is_empty() {
        anyhow::bail!(
            "No aliases matched pattern \"{pattern}\".\n\
             Check current aliases with: denki aliases"
        );
    }

    if dry_run {
        println!(
            "Would turn {} {} alias(es) matching \"{}\":",
            action.as_verb(),
            matches.len(),
            pattern
        );
        for (alias, entry) in matches {
            println!("  {} [{}; {}]", alias.bold(), entry.ip, entry.protocol);
        }
        return Ok(());
    }

    println!(
        "Turning {} {} alias(es) matching \"{}\" (up to {} at once):",
        action.as_verb(),
        matches.len(),
        pattern,
        concurrency
    );

    let results = execute_group(matches, action, concurrency, &LiveDeviceTransport).await;
    let total = results.len();
    let mut failures = Vec::new();
    for outcome in results {
        match outcome.result {
            Ok(on) => println!(
                "  {} {} -> {}",
                "OK".green().bold(),
                outcome.alias,
                if on { "on" } else { "off" }
            ),
            Err(error) => {
                eprintln!("  {} {}: {error:#}", "FAILED".red().bold(), outcome.alias);
                failures.push(outcome.alias);
            }
        }
    }

    let succeeded = total - failures.len();
    println!(
        "Completed: {} succeeded, {} failed.",
        succeeded.to_string().green(),
        failures.len().to_string().red()
    );
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "Group action failed for {} alias(es): {}",
            failures.len(),
            failures.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;
    use std::sync::Mutex;

    struct FakeTransport {
        fail_alias: Option<&'static str>,
        calls: Mutex<Vec<String>>,
    }

    impl DeviceTransport for FakeTransport {
        async fn apply_power(&self, target: &Resolved, action: GroupAction) -> Result<bool> {
            let alias = target.saved_name.clone().unwrap_or_default();
            self.calls.lock().unwrap().push(alias.clone());
            if self.fail_alias == Some(alias.as_str()) {
                anyhow::bail!("simulated transport failure");
            }
            Ok(!matches!(action, GroupAction::Off))
        }
    }

    fn group_target(alias: &str, ip: &str) -> (String, hosts::HostEntry) {
        group_target_with_protocol(alias, ip, hosts::Protocol::Kasa)
    }

    fn group_target_with_protocol(
        alias: &str,
        ip: &str,
        protocol: hosts::Protocol,
    ) -> (String, hosts::HostEntry) {
        (
            alias.to_string(),
            hosts::HostEntry {
                ip: ip.to_string(),
                protocol,
            },
        )
    }

    #[test]
    fn toggle_target_bulb_on_returns_false() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"light_state": {"on_off": 1}}}});
        assert!(!toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_bulb_off_returns_true() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"light_state": {"on_off": 0}}}});
        assert!(toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_plug_on_returns_false() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 1}}});
        assert!(!toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_plug_off_returns_true() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 0}}});
        assert!(toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_dimmer_on_returns_false() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 1}}});
        assert!(!toggle_target(&DeviceKind::Dimmer, &json));
    }

    #[test]
    fn toggle_target_dimmer_off_returns_true() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 0}}});
        assert!(toggle_target(&DeviceKind::Dimmer, &json));
    }

    #[test]
    fn toggle_target_strip_any_on_returns_false() {
        let json = test_support::strip_sysinfo(
            "Strip",
            "HS300(US)",
            "TIM",
            vec![
                test_support::strip_child("A1", 1, "Outlet 1", 0),
                test_support::strip_child("A2", 0, "Outlet 2", 0),
            ],
        );
        assert!(!toggle_target(&DeviceKind::Strip, &json));
    }

    #[test]
    fn toggle_target_strip_all_off_returns_true() {
        let json = test_support::strip_sysinfo(
            "Strip",
            "HS300(US)",
            "TIM",
            vec![
                test_support::strip_child("A1", 0, "Outlet 1", 0),
                test_support::strip_child("A2", 0, "Outlet 2", 0),
            ],
        );
        assert!(toggle_target(&DeviceKind::Strip, &json));
    }

    #[tokio::test]
    async fn group_executor_continues_after_transport_failure() {
        let transport = FakeTransport {
            fail_alias: Some("broken lamp"),
            calls: Mutex::new(Vec::new()),
        };
        let results = execute_group(
            vec![
                group_target("first lamp", "192.0.2.1"),
                group_target("broken lamp", "192.0.2.2"),
                group_target("last lamp", "192.0.2.3"),
            ],
            GroupAction::Off,
            2,
            &transport,
        )
        .await;

        assert_eq!(results.len(), 3);
        assert_eq!(results.iter().filter(|r| r.result.is_ok()).count(), 2);
        assert_eq!(results.iter().filter(|r| r.result.is_err()).count(), 1);
        let calls = transport.calls.lock().unwrap();
        assert!(calls.contains(&"last lamp".to_string()));
    }

    #[tokio::test]
    async fn group_executor_reports_resulting_power_state() {
        let transport = FakeTransport {
            fail_alias: None,
            calls: Mutex::new(Vec::new()),
        };
        let results = execute_group(
            vec![group_target("office", "192.0.2.4")],
            GroupAction::On,
            1,
            &transport,
        )
        .await;

        assert_eq!(results[0].result.as_ref().unwrap(), &true);
    }

    struct FailureMatrixTransport;

    impl DeviceTransport for FailureMatrixTransport {
        async fn apply_power(&self, target: &Resolved, _: GroupAction) -> Result<bool> {
            match target.saved_name.as_deref() {
                Some("timeout") => anyhow::bail!("simulated timeout"),
                Some("malformed") => anyhow::bail!("simulated malformed response"),
                _ => Ok(target.protocol == hosts::Protocol::Klap),
            }
        }
    }

    #[tokio::test]
    async fn group_executor_handles_mixed_protocols_and_distinct_failures() {
        let results = execute_group(
            vec![
                group_target("kasa", "192.0.2.10"),
                group_target_with_protocol("tapo", "192.0.2.11", hosts::Protocol::Klap),
                group_target("timeout", "192.0.2.12"),
                group_target("malformed", "192.0.2.13"),
            ],
            GroupAction::Toggle,
            3,
            &FailureMatrixTransport,
        )
        .await;

        assert_eq!(results.len(), 4);
        assert_eq!(
            results
                .iter()
                .filter(|result| result.result.is_ok())
                .count(),
            2
        );
        let errors = results
            .iter()
            .filter_map(|result| result.result.as_ref().err())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(errors.contains("timeout"));
        assert!(errors.contains("malformed"));
        assert_eq!(
            results
                .iter()
                .find(|result| result.alias == "tapo")
                .unwrap()
                .result
                .as_ref()
                .unwrap(),
            &true
        );
    }
}
