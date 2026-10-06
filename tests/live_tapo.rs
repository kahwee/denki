//! Opt-in P125 power round trip. Never runs in the ordinary offline suite.
use anyhow::{Context, Result, bail};
use std::time::Duration;

#[path = "support/power_cycle.rs"]
mod power_cycle;
use power_cycle::{Plug, State, round_trip};

struct LivePlug {
    ip: String,
    expected_name: Option<String>,
}

impl Plug for LivePlug {
    async fn read(&mut self) -> Result<State> {
        let info = denki::tapo_client::info(&self.ip).await?;
        if info.model.split('(').next().unwrap_or_default().trim() != "P125" {
            bail!("Live test requires a P125; no power command was sent");
        }
        if self
            .expected_name
            .as_ref()
            .is_some_and(|name| name != &info.nickname)
        {
            bail!("Device name differs from DENKI_TAPO_TEST_NAME; refusing power writes");
        }
        println!(
            "Read P125: HW {}, FW {}, power {}",
            info.hw_ver.escape_default(),
            info.fw_ver.escape_default(),
            if info.device_on { "on" } else { "off" }
        );
        Ok(State {
            id: info.device_id,
            on: info.device_on,
        })
    }

    async fn set(&mut self, on: bool) -> Result<()> {
        denki::tapo_client::set_power(&self.ip, Some(on)).await?;
        Ok(())
    }
}

#[tokio::test]
#[ignore = "changes P125 power; requires explicit DENKI_TAPO_TEST_IP"]
async fn p125_power_round_trip() -> Result<()> {
    let ip = std::env::var("DENKI_TAPO_TEST_IP").context(
        "Set DENKI_TAPO_TEST_IP to the explicitly selected P125; there is no default target",
    )?;
    ip.parse::<std::net::IpAddr>()
        .context("DENKI_TAPO_TEST_IP must be an IP address")?;
    let mut plug = LivePlug {
        ip,
        expected_name: std::env::var("DENKI_TAPO_TEST_NAME").ok(),
    };
    round_trip(&mut plug, Duration::from_millis(500)).await?;
    println!(
        "P125: both power states read back successfully; original state restored. This verifies reported relay state, not physical load behavior."
    );
    Ok(())
}
