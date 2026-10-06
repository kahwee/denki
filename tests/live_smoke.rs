use anyhow::{Context, Result, bail};
use std::env;
use std::process::Command;

fn denki_bin() -> String {
    env::var("CARGO_BIN_EXE_denki").unwrap_or_else(|_| "target/debug/denki".to_string())
}

fn parse_targets(var: &str, default: &[&str]) -> Vec<String> {
    match env::var(var) {
        Ok(value) if !value.trim().is_empty() => value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => default.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn run_denki(args: &[&str]) -> Result<String> {
    let output = Command::new(denki_bin())
        .args(args)
        .output()
        .with_context(|| format!("failed to launch denki with args: {args:?}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "denki {:?} failed with status {}.\nstdout:\n{}\nstderr:\n{}",
            args,
            output
                .status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string()),
            stdout,
            stderr
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[path = "support/power_cycle.rs"]
mod power_cycle;
use power_cycle::{Plug, State, round_trip};

struct Device {
    target: String,
}
impl Plug for Device {
    async fn read(&mut self) -> Result<State> {
        let resolved = denki::resolve::resolve_quiet(&self.target)?;
        match resolved.protocol {
            denki::hosts::Protocol::Kasa => {
                let json = denki::ops::sysinfo(&resolved.ip).await?;
                if json.pointer("/system/get_sysinfo/children").is_some() {
                    bail!(
                        "Power strips require per-outlet restoration; this harness refuses to cycle them"
                    );
                }
                let id = json
                    .pointer("/system/get_sysinfo/deviceId")
                    .and_then(serde_json::Value::as_str)
                    .context("Missing device identity")?
                    .to_owned();
                Ok(State {
                    id,
                    on: denki::ops::kasa_power_state(&json)?,
                })
            }
            denki::hosts::Protocol::Klap => {
                let (user, pass) = denki::creds::load()?;
                let mut session = denki::klap::handshake(&resolved.ip, &user, &pass).await?;
                let json = denki::ops::tapo_device_info(&mut session).await?;
                let info = denki::tapo::parse(&json).context("Invalid Tapo info")?;
                Ok(State {
                    id: info.device_id,
                    on: info.device_on,
                })
            }
            denki::hosts::Protocol::Tapo => {
                let info = denki::tapo_client::info(&resolved.ip).await?;
                Ok(State {
                    id: info.device_id,
                    on: info.device_on,
                })
            }
        }
    }
    async fn set(&mut self, on: bool) -> Result<()> {
        if on {
            denki::commands::handle_on(&self.target, None).await
        } else {
            denki::commands::handle_off(&self.target, None).await
        }
    }
}

#[tokio::test]
#[ignore = "changes device power; requires explicit DENKI_SMOKE_POWER_TARGETS"]
async fn power_cycle_selected_targets() -> Result<()> {
    let targets = parse_targets("DENKI_SMOKE_POWER_TARGETS", &[]);
    if targets.is_empty() {
        bail!("Set DENKI_SMOKE_POWER_TARGETS; there is no default target");
    }
    for target in targets {
        round_trip(
            &mut Device { target },
            std::time::Duration::from_millis(500),
        )
        .await?;
    }
    Ok(())
}

#[test]
#[ignore = "requires a live light strip alias in DENKI_SMOKE_LIGHTSTRIP"]
fn light_strip_effect_cycle() -> Result<()> {
    let target = match env::var("DENKI_SMOKE_LIGHTSTRIP") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(()),
    };

    let effects_output = run_denki(&["effects", &target])?;
    assert!(
        effects_output.contains("Available effects:")
            || effects_output.contains("Built-in effects:"),
        "expected effect catalog output:\n{effects_output}"
    );

    run_denki(&["effect", &target, "Rainbow"])?;
    run_denki(&["effect", &target, "Off"])?;
    Ok(())
}
