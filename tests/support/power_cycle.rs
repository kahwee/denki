use anyhow::{Context, Result, bail};
use std::time::Duration;

#[derive(Clone)]
pub struct State {
    pub id: String,
    pub on: bool,
}

pub trait Plug {
    async fn read(&mut self) -> Result<State>;
    async fn set(&mut self, on: bool) -> Result<()>;
}

fn check_identity(actual: &State, original: &State) -> Result<()> {
    if actual.id.is_empty() || actual.id != original.id {
        return Err(denki::error::error(
            "identity_mismatch",
            "Device identity changed; refusing further power writes",
        ));
    }
    Ok(())
}

pub(crate) async fn wait_for(
    plug: &mut impl Plug,
    original: &State,
    on: bool,
    delay: Duration,
) -> Result<()> {
    // Only successful reads with a stale state are polled. Errors stop immediately.
    for attempt in 0..3 {
        let actual = plug.read().await?;
        check_identity(&actual, original)?;
        if actual.on == on {
            return Ok(());
        }
        if attempt < 2 {
            tokio::time::sleep(delay).await;
        }
    }
    bail!(
        "State readback did not reach the requested {} state",
        if on { "on" } else { "off" }
    );
}

fn authentication_failure(error: &anyhow::Error) -> bool {
    matches!(
        denki::error::code(error),
        "tpap_credentials" | "tpap_auth_attempts_limit" | "authentication_failed"
    )
}

pub async fn round_trip(plug: &mut impl Plug, delay: Duration) -> Result<()> {
    let original = plug
        .read()
        .await
        .context("initial read failed; no writes attempted")?;
    if original.id.is_empty() {
        bail!("Missing device identity; no writes attempted");
    }
    let cycle: Result<()> = async {
        plug.set(!original.on).await?;
        wait_for(plug, &original, !original.on, delay).await?;
        plug.set(original.on).await?;
        wait_for(plug, &original, original.on, delay).await?;
        Ok(())
    }
    .await;
    if let Err(error) = cycle {
        if authentication_failure(&error) || denki::error::code(&error) == "identity_mismatch" {
            bail!(
                "Cycle failed: {error:#}. Restoration skipped after an authentication or identity failure; check power manually."
            );
        }
        // Never blindly write after an uncertain failure: check identity and state first.
        let restore: Result<()> = async {
            let actual = plug.read().await?;
            check_identity(&actual, &original)?;
            if actual.on != original.on {
                plug.set(original.on).await?;
            }
            wait_for(plug, &original, original.on, delay).await
        }
        .await;
        match restore {
            Ok(()) => bail!("Cycle failed: {error:#}. Original state restored and verified."),
            Err(restore_error) => bail!(
                "Cycle failed: {error:#}. Restoration failed: {restore_error:#}. Check power manually."
            ),
        }
    }
    Ok(())
}
