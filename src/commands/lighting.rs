use anyhow::Result;

use crate::bulb;
use crate::devices::{self, DeviceKind};
use crate::dimmer;
use crate::ops;

use super::shared::KasaContext;

async fn ensure_bulb_on(ctx: &KasaContext) -> Result<()> {
    if bulb::parse(ctx.json()).is_some_and(|b| !b.light_state.is_on()) {
        ops::bulb_on(ctx.ip()).await?;
    }
    Ok(())
}

pub async fn handle_dim(host: &str, level: u8) -> Result<()> {
    let ctx = KasaContext::load(host, "dim").await?;
    devices::can_dim(ctx.kind())?;
    match ctx.kind() {
        DeviceKind::Bulb => {
            ensure_bulb_on(&ctx).await?;
            ops::bulb_set_brightness(ctx.ip(), level).await?;
        }
        DeviceKind::Dimmer => {
            if level > 0 && dimmer::parse(ctx.json()).is_some_and(|d| !d.is_on()) {
                ops::relay_on(ctx.ip()).await?;
            }
            ops::dimmer_set_brightness(ctx.ip(), level).await?;
        }
        other => anyhow::bail!("`dim` is only supported on bulbs and dimmers, not {other}"),
    }
    println!("Brightness -> {level}%");
    Ok(())
}

pub async fn handle_color_temp(host: &str, kelvin: u16) -> Result<()> {
    let ctx = KasaContext::load(host, "color-temp").await?;
    devices::can_set_color_temp(ctx.kind())?;
    ensure_bulb_on(&ctx).await?;
    ops::bulb_set_color_temp(ctx.ip(), kelvin).await?;
    println!("Color temperature -> {kelvin}K");
    Ok(())
}

pub async fn handle_color(host: &str, hue: u16, saturation: u8, value: u8) -> Result<()> {
    let ctx = KasaContext::load(host, "color").await?;
    devices::can_set_color(ctx.kind())?;
    ensure_bulb_on(&ctx).await?;
    ops::bulb_set_color(ctx.ip(), hue, saturation, value).await?;
    println!("Color -> hue:{hue} sat:{saturation} val:{value}");
    Ok(())
}
