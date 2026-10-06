//! Upstream Tapo plug adapter: negotiates TPAP/KLAP and reuses Denki credentials.
//! Existing explicit KLAP aliases continue to use `crate::klap`.

use std::future::Future;
use std::time::Duration;

use ::tapo::{ApiClient, Error, PlugHandler, responses::DeviceInfoPlugResult};
use anyhow::Result;

use crate::tapo::TapoDevice;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

mod error;
use error::upstream_error;

async fn bounded<T>(stage: &str, future: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(OPERATION_TIMEOUT, future)
        .await
        .map_err(|_| {
            crate::error::error(
                "timeout",
                format!("Tapo {stage} timed out after 30s; no automatic retry was made"),
            )
        })?
}

async fn connect(ip: &str) -> Result<PlugHandler> {
    let (user, pass) = crate::creds::load()?;
    ApiClient::new(user, pass)
        .with_timeout(Duration::from_secs(10))
        .p100(ip)
        .await
        .map_err(|error| upstream_error("connection/authentication", error))
}

fn normalize(info: DeviceInfoPlugResult) -> TapoDevice {
    // Upstream has already decoded nickname. Do not use the KLAP JSON parser.
    TapoDevice {
        model: info.model,
        hw_ver: info.hw_ver,
        fw_ver: info.fw_ver,
        device_on: info.device_on,
        on_time: info.on_time,
        rssi: i32::from(info.rssi),
        signal_level: info.signal_level,
        nickname: info.nickname,
        overheated: false, // Not exposed by the generic plug response.
        device_id: info.device_id,
    }
}

/// Read information without changing power or settings.
pub(crate) async fn probe_info(ip: &str) -> Result<TapoDevice> {
    bounded("information read", async {
        let device = connect(ip).await?;
        let info = device
            .get_device_info()
            .await
            .map_err(|error| upstream_error("information read", error))?;
        if info.device_id.trim().is_empty() {
            return Err(crate::error::malformed("Missing Tapo device identity"));
        }
        Ok(normalize(info))
    })
    .await
}

/// Read information and check any identity bound to this saved address.
pub async fn info(ip: &str) -> Result<TapoDevice> {
    let info = probe_info(ip).await?;
    crate::hosts::verify_identity(ip, crate::hosts::Protocol::Tapo, Some(&info.device_id))?;
    Ok(info)
}

struct CheckedPlug<'a> {
    device: PlugHandler,
    ip: &'a str,
}

trait PlugIo {
    async fn read_info(&self) -> std::result::Result<DeviceInfoPlugResult, Error>;
    async fn write_power(&self, on: bool) -> std::result::Result<(), Error>;
}

impl PlugIo for CheckedPlug<'_> {
    async fn read_info(&self) -> std::result::Result<DeviceInfoPlugResult, Error> {
        let info = self.device.get_device_info().await?;
        if info.device_id.trim().is_empty() {
            return Err(Error::Other(crate::error::malformed(
                "Missing Tapo device identity",
            )));
        }
        crate::hosts::verify_identity(self.ip, crate::hosts::Protocol::Tapo, Some(&info.device_id))
            .map_err(Error::Other)?;
        Ok(info)
    }

    async fn write_power(&self, on: bool) -> std::result::Result<(), Error> {
        if on {
            self.device.on().await
        } else {
            self.device.off().await
        }
    }
}

async fn apply_power(device: &impl PlugIo, on: Option<bool>) -> Result<bool> {
    let before = device
        .read_info()
        .await
        .map_err(|error| upstream_error("power capability read", error))?;
    crate::devices::require_tapo_auto_power(&before.model, &before.r#type)?;
    let on = on.unwrap_or(!before.device_on);
    device
        .write_power(on)
        .await
        .map_err(|error| upstream_error("power write (read info before retrying)", error))?;
    let after = device
        .read_info()
        .await
        .map_err(|error| upstream_error("power readback (write was acknowledged)", error))?;
    if after.device_id != before.device_id {
        return Err(crate::error::error(
            "identity_mismatch",
            "Tapo power readback found a different device; no retry was made",
        ));
    }
    if after.device_on != on {
        return Err(crate::error::error(
            "state_mismatch",
            "Tapo power readback did not confirm the requested state; no retry was made",
        ));
    }
    Ok(on)
}

/// Set power, or toggle when `on` is `None`. Check capabilities before writing.
/// Writes are never retried; a timeout leaves the resulting state uncertain.
pub async fn set_power(ip: &str, on: Option<bool>) -> Result<bool> {
    bounded(
        "power operation (state may be uncertain; read info before retrying)",
        async {
            let device = CheckedPlug {
                device: connect(ip).await?,
                ip,
            };
            apply_power(&device, on).await
        },
    )
    .await
}

#[cfg(test)]
mod tests;
