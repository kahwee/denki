//! Device operations — one function per API call, grouped by device type.
//!
//! Color-bulb energy: use smartlife.iot.common.emeter, NOT bare "emeter" (that returns -2001).
//! LED control: set_led_off with off:0 = LED on, off:1 = LED off (inverted naming).

use crate::klap::KlapSession;
use crate::transport;
use anyhow::Result;
use serde_json::json;

pub async fn sysinfo(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"system": {"get_sysinfo": {}}})).await
}

async fn bulb_set_power(host: &str, on: bool) -> Result<()> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {
            "transition_light_state": {"on_off": u8::from(on), "transition_period": 0}
        }}),
    )
    .await?;
    Ok(())
}

pub async fn bulb_on(host: &str) -> Result<()> {
    bulb_set_power(host, true).await
}

pub async fn bulb_off(host: &str) -> Result<()> {
    bulb_set_power(host, false).await
}

pub async fn bulb_set_brightness(host: &str, level: u8) -> Result<()> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {
            "transition_light_state": {"brightness": level, "transition_period": 0}
        }}),
    )
    .await?;
    Ok(())
}

/// hue/saturation must be 0 to clear any previous color mode state on the device.
pub async fn bulb_set_color_temp(host: &str, kelvin: u16) -> Result<()> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {
            "transition_light_state": {
                "color_temp": kelvin,
                "hue": 0,
                "saturation": 0,
                "transition_period": 0
            }
        }}),
    )
    .await?;
    Ok(())
}

/// color_temp must be 0 to activate color mode; some firmware ignores hue/saturation otherwise.
pub async fn bulb_set_color(host: &str, hue: u16, saturation: u8, value: u8) -> Result<()> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {
            "transition_light_state": {
                "hue": hue,
                "saturation": saturation,
                "brightness": value,
                "color_temp": 0,
                "transition_period": 0
            }
        }}),
    )
    .await?;
    Ok(())
}

pub async fn bulb_specs(host: &str) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {"get_light_details": {}}}),
    )
    .await
}

pub async fn bulb_presets(host: &str) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {"get_preferred_state": {}}}),
    )
    .await
}

// Color bulbs use smartlife.iot.common.emeter — bare "emeter" returns -2001 on bulbs.
pub async fn bulb_energy(host: &str) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.common.emeter": {"get_realtime": {}}}),
    )
    .await
}

pub async fn bulb_energy_daily(host: &str, year: u16, month: u8) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.common.emeter": {
            "get_daystat": {"month": month, "year": year}
        }}),
    )
    .await
}

pub async fn bulb_energy_monthly(host: &str, year: u16) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.common.emeter": {"get_monthstat": {"year": year}}}),
    )
    .await
}

// HS220 rejects brightness=0 — route to relay_off instead.
pub async fn dimmer_set_brightness(host: &str, level: u8) -> Result<()> {
    if level == 0 {
        return relay_off(host).await;
    }
    transport::send(
        host,
        json!({"smartlife.iot.dimmer": {"set_brightness": {"brightness": level}}}),
    )
    .await?;
    Ok(())
}

pub async fn relay_on(host: &str) -> Result<()> {
    transport::send(host, json!({"system": {"set_relay_state": {"state": 1}}})).await?;
    Ok(())
}

pub async fn relay_off(host: &str) -> Result<()> {
    transport::send(host, json!({"system": {"set_relay_state": {"state": 0}}})).await?;
    Ok(())
}

// set_led_off is inverted: off:0 = LED lit, off:1 = LED dark.
pub async fn device_led(host: &str, on: bool) -> Result<()> {
    transport::send(
        host,
        json!({"system": {"set_led_off": {"off": if on { 0 } else { 1 }}}}),
    )
    .await?;
    Ok(())
}

pub async fn device_energy(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"emeter": {"get_realtime": {}}})).await
}

pub async fn device_energy_daily(host: &str, year: u16, month: u8) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"emeter": {"get_daystat": {"month": month, "year": year}}}),
    )
    .await
}

pub async fn device_energy_monthly(host: &str, year: u16) -> Result<serde_json::Value> {
    transport::send(host, json!({"emeter": {"get_monthstat": {"year": year}}})).await
}

pub async fn device_schedules(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"schedule": {"get_rules": {}}})).await
}

pub async fn device_time(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"time": {"get_time": {}}})).await
}

// Individual outlets are addressed via context.child_ids; callers resolve outlet number → child id from sysinfo.
async fn strip_outlet_set(host: &str, child_id: &str, on: bool) -> Result<()> {
    transport::send(
        host,
        json!({
            "context": {"child_ids": [child_id]},
            "system": {"set_relay_state": {"state": u8::from(on)}}
        }),
    )
    .await?;
    Ok(())
}

pub async fn strip_outlet_on(host: &str, child_id: &str) -> Result<()> {
    strip_outlet_set(host, child_id, true).await
}

pub async fn strip_outlet_off(host: &str, child_id: &str) -> Result<()> {
    strip_outlet_set(host, child_id, false).await
}

pub async fn strip_outlet_energy(host: &str, child_id: &str) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({
            "context": {"child_ids": [child_id]},
            "emeter": {"get_realtime": {}}
        }),
    )
    .await
}

pub async fn strip_outlet_energy_daily(
    host: &str,
    child_id: &str,
    year: u16,
    month: u8,
) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({
            "context": {"child_ids": [child_id]},
            "emeter": {"get_daystat": {"month": month, "year": year}}
        }),
    )
    .await
}

pub async fn strip_outlet_energy_monthly(
    host: &str,
    child_id: &str,
    year: u16,
) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({
            "context": {"child_ids": [child_id]},
            "emeter": {"get_monthstat": {"year": year}}
        }),
    )
    .await
}

pub async fn strip_outlet_rename(host: &str, child_id: &str, name: &str) -> Result<()> {
    transport::send(
        host,
        json!({
            "context": {"child_ids": [child_id]},
            "system": {"set_dev_alias": {"alias": name}}
        }),
    )
    .await?;
    Ok(())
}

pub async fn rename(host: &str, name: &str) -> Result<()> {
    transport::send(host, json!({"system": {"set_dev_alias": {"alias": name}}})).await?;
    Ok(())
}

pub async fn restart(host: &str) -> Result<()> {
    transport::send(host, json!({"system": {"reboot": {"delay": 1}}})).await?;
    Ok(())
}

pub async fn tapo_device_info(session: &mut KlapSession) -> Result<serde_json::Value> {
    session
        .send(&serde_json::to_string(
            &json!({"method": "get_device_info", "params": {}}),
        )?)
        .await
}

async fn tapo_set_power(session: &mut KlapSession, on: bool) -> Result<()> {
    let resp = session
        .send(&serde_json::to_string(
            &json!({"method": "set_device_info", "params": {"device_on": on}}),
        )?)
        .await?;
    check_tapo_error(&resp)
}

pub async fn tapo_on(session: &mut KlapSession) -> Result<()> {
    tapo_set_power(session, true).await
}

pub async fn tapo_off(session: &mut KlapSession) -> Result<()> {
    tapo_set_power(session, false).await
}

pub async fn tapo_toggle(session: &mut KlapSession) -> Result<bool> {
    let info = tapo_device_info(session).await?;
    let is_on = info
        .pointer("/result/device_on")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_on {
        tapo_off(session).await?;
        Ok(false)
    } else {
        tapo_on(session).await?;
        Ok(true)
    }
}

fn check_tapo_error(resp: &serde_json::Value) -> Result<()> {
    let code = resp.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code != 0 {
        anyhow::bail!("Tapo device error: code {code}");
    }
    Ok(())
}
