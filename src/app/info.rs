use anyhow::Result;

use crate::hosts;
use crate::ops;
use crate::resolve::resolve_quiet;
use crate::tapo;

use super::shared::{print_kasa_detail, tapo_session};

pub(super) async fn handle_info(host: String) -> Result<()> {
    let r = resolve_quiet(&host)?;
    let hint = r.saved_name.as_deref().unwrap_or(&r.ip).to_string();
    match r.protocol {
        hosts::Protocol::Tapo => {
            let device = crate::tapo_client::info(&r.ip).await?;
            crate::output::record(
                serde_json::json!({"ip": r.ip, "protocol": "tapo", "device": crate::output::sanitized(serde_json::to_value(&device)?)}),
            );
            crate::display::print_tapo_detail(&r.ip, &device, &hint);
        }
        hosts::Protocol::Klap => {
            let mut session = tapo_session(&r.ip).await?;
            let json = ops::tapo_device_info(&mut session).await?;
            crate::output::record(
                serde_json::json!({"ip": r.ip, "protocol": "klap", "device": crate::output::sanitized(json["result"].clone())}),
            );
            match tapo::parse(&json) {
                Some(d) => crate::display::print_tapo_detail(&r.ip, &d, &hint),
                None => anyhow::bail!("Could not parse Tapo device info from {}", r.ip),
            }
        }
        hosts::Protocol::Kasa => {
            let json = ops::sysinfo(&r.ip).await?;
            crate::output::record(
                serde_json::json!({"ip": r.ip, "protocol": "kasa", "device": crate::output::sanitized(json["system"]["get_sysinfo"].clone())}),
            );
            print_kasa_detail(&r.ip, &json, &hint)?;
        }
    }
    Ok(())
}
