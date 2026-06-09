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
        hosts::Protocol::Klap => {
            let mut session = tapo_session(&r.ip).await?;
            let json = ops::tapo_device_info(&mut session).await?;
            match tapo::parse(&json) {
                Some(d) => crate::display::print_tapo_detail(&r.ip, &d, &hint),
                None => anyhow::bail!("Could not parse Tapo device info from {}", r.ip),
            }
        }
        hosts::Protocol::Kasa => {
            let json = ops::sysinfo(&r.ip).await?;
            print_kasa_detail(&r.ip, &json, &hint)?;
        }
    }
    Ok(())
}
