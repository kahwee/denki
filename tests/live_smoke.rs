use anyhow::{bail, Context, Result};
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

#[test]
#[ignore = "requires live TP-Link devices on the local network"]
fn power_cycle_default_targets() -> Result<()> {
    let targets = parse_targets(
        "DENKI_SMOKE_POWER_TARGETS",
        &["Living Room Right Lamp"],
    );

    for target in targets {
        let on_output = run_denki(&["on", &target])?;
        assert!(
            on_output.contains(" on"),
            "expected an on confirmation in output:\n{on_output}"
        );

        let off_output = run_denki(&["off", &target])?;
        assert!(
            off_output.contains(" off"),
            "expected an off confirmation in output:\n{off_output}"
        );
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
        effects_output.contains("Available effects:") || effects_output.contains("Built-in effects:"),
        "expected effect catalog output:\n{effects_output}"
    );

    run_denki(&["effect", &target, "Rainbow"])?;
    run_denki(&["effect", &target, "Off"])?;
    Ok(())
}
