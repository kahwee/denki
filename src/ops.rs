//! Device operations — one function per API call, grouped by device type.
//!
//! Bulb/light-strip energy: use smartlife.iot.common.emeter, NOT bare "emeter" (that returns -2001 on bulbs/light strips).
//! LED control: set_led_off with off:0 = LED on, off:1 = LED off (inverted naming).

use crate::bulb::LightingEffectState;
use crate::klap::KlapSession;
use crate::transport;
use anyhow::Result;
use serde::Serialize;
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

fn lightstrip_state_payload(state: serde_json::Value) -> serde_json::Value {
    json!({"smartlife.iot.lightStrip": {"set_light_state": state}})
}

async fn lightstrip_set_state(host: &str, state: serde_json::Value) -> Result<()> {
    transport::send(host, lightstrip_state_payload(state)).await?;
    Ok(())
}

pub async fn lightstrip_set_power(host: &str, on: bool) -> Result<()> {
    lightstrip_set_state(
        host,
        json!({"on_off": u8::from(on), "transition_period": 0}),
    )
    .await
}

pub async fn lightstrip_set_brightness(host: &str, level: u8) -> Result<()> {
    if level == 0 {
        return lightstrip_set_power(host, false).await;
    }
    lightstrip_set_state(
        host,
        json!({
            "brightness": level,
            "on_off": 1,
            "ignore_default": 1,
            "transition_period": 0
        }),
    )
    .await
}

pub async fn lightstrip_set_color_temp(host: &str, kelvin: u16) -> Result<()> {
    lightstrip_set_state(
        host,
        json!({
            "color_temp": kelvin,
            "hue": 0,
            "saturation": 0,
            "on_off": 1,
            "ignore_default": 1,
            "transition_period": 0
        }),
    )
    .await
}

pub async fn lightstrip_set_color(host: &str, hue: u16, saturation: u8, value: u8) -> Result<()> {
    lightstrip_set_state(
        host,
        json!({
            "hue": hue,
            "saturation": saturation,
            "brightness": value,
            "color_temp": 0,
            "on_off": u8::from(value > 0),
            "ignore_default": 1,
            "transition_period": 0
        }),
    )
    .await
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

// Bulbs and light strips use smartlife.iot.common.emeter — bare "emeter" returns -2001 on bulbs/light strips.
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

pub async fn lightstrip_current_effect(host: &str) -> Result<LightingEffectState> {
    transport::send(
        host,
        json!({"smartlife.iot.lighting_effect": {"get_lighting_effect": {}}}),
    )
    .await
    .and_then(|value| {
        serde_json::from_value::<LightingEffectState>(
            value
                .pointer("/smartlife.iot.lighting_effect/get_lighting_effect")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("light strip did not return effect details"))?,
        )
        .map_err(Into::into)
    })
}

#[derive(Serialize)]
struct LightstripEffectRequest<'a> {
    enable: u8,
    name: &'a str,
    custom: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segments: Option<&'a [u8]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expansion_strategy: Option<u8>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    effect_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hue_range: Option<&'a [u16]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    saturation_range: Option<&'a [u8]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brightness_range: Option<&'a [u8]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transition: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transition_range: Option<&'a [u16]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    init_states: Option<&'a [Vec<u8>]>,
}

fn lightstrip_effect_request<'a>(
    effect: &'a LightingEffectState,
    name: &'a str,
    enable: u8,
) -> LightstripEffectRequest<'a> {
    LightstripEffectRequest {
        enable,
        name,
        custom: effect.custom,
        id: effect.id.as_deref(),
        brightness: effect.brightness,
        segments: effect.segments.as_deref(),
        expansion_strategy: effect.expansion_strategy,
        effect_type: effect.effect_type.as_deref(),
        hue_range: effect.hue_range.as_deref(),
        saturation_range: effect.saturation_range.as_deref(),
        brightness_range: effect.brightness_range.as_deref(),
        duration: effect.duration,
        transition: effect.transition,
        transition_range: effect.transition_range.as_deref(),
        init_states: effect.init_states.as_deref(),
    }
}

fn lightstrip_effect_payload(
    effect: &LightingEffectState,
    name: &str,
    enable: u8,
) -> Result<serde_json::Value> {
    Ok(json!({
        "smartlife.iot.lighting_effect": {
            "set_lighting_effect": serde_json::to_value(lightstrip_effect_request(effect, name, enable))?
        }
    }))
}

pub async fn lightstrip_set_effect(
    host: &str,
    effect: &LightingEffectState,
    name: &str,
) -> Result<()> {
    transport::send(host, lightstrip_effect_payload(effect, name, 1)?).await?;
    Ok(())
}

pub async fn lightstrip_disable_effect(host: &str) -> Result<()> {
    let mut effect = lightstrip_current_effect(host).await?;
    effect.enable = 0;
    transport::send(host, lightstrip_effect_payload(&effect, &effect.name, 0)?).await?;
    Ok(())
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
        json!({"system": {"set_led_off": {"off": i32::from(!on)}}}),
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

pub async fn tapo_energy_usage(session: &mut KlapSession) -> Result<serde_json::Value> {
    let response = session
        .send(&serde_json::to_string(
            &json!({"method": "get_energy_usage", "params": {}}),
        )?)
        .await?;
    check_tapo_error(&response)?;
    Ok(response)
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
        .and_then(serde_json::Value::as_bool)
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
    let code = resp
        .get("error_code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    if code != 0 {
        anyhow::bail!("Tapo device error: code {code}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn effect() -> LightingEffectState {
        LightingEffectState {
            enable: 0,
            name: "Flicker".to_string(),
            custom: 0,
            id: Some("TapoStrip_4HVKmMc6vEzjm36jXaGwMs".to_string()),
            brightness: Some(100),
            segments: Some(vec![1]),
            expansion_strategy: Some(1),
            effect_type: Some("random".to_string()),
            hue_range: Some(vec![30, 40]),
            saturation_range: Some(vec![100, 100]),
            brightness_range: Some(vec![50, 100]),
            duration: Some(0),
            transition: Some(0),
            transition_range: Some(vec![375, 500]),
            init_states: Some(vec![vec![30, 81, 80]]),
        }
    }

    #[test]
    fn lightstrip_effect_payload_overrides_enable_and_name() {
        let payload = lightstrip_effect_payload(&effect(), "Rainbow", 1).unwrap();
        let inner = payload
            .pointer("/smartlife.iot.lighting_effect/set_lighting_effect")
            .expect("payload should contain set_lighting_effect");
        assert_eq!(inner["enable"], json!(1));
        assert_eq!(inner["name"], json!("Rainbow"));
        assert_eq!(inner["id"], json!("TapoStrip_4HVKmMc6vEzjm36jXaGwMs"));
        assert_eq!(inner["brightness"], json!(100));
        assert_eq!(inner["segments"], json!([1]));
        assert_eq!(inner["transition_range"], json!([375, 500]));
    }

    #[test]
    fn lightstrip_effect_payload_can_disable_without_losing_descriptor() {
        let payload = lightstrip_effect_payload(&effect(), "Flicker", 0).unwrap();
        let inner = payload
            .pointer("/smartlife.iot.lighting_effect/set_lighting_effect")
            .expect("payload should contain set_lighting_effect");
        assert_eq!(inner["enable"], json!(0));
        assert_eq!(inner["name"], json!("Flicker"));
        assert_eq!(inner["custom"], json!(0));
        assert_eq!(inner["type"], json!("random"));
        assert_eq!(inner["init_states"], json!([[30, 81, 80]]));
    }

    #[test]
    fn lightstrip_effect_payload_serializes_without_nulls() {
        let payload = lightstrip_effect_payload(&effect(), "Rainbow", 1).unwrap();
        let inner = payload
            .pointer("/smartlife.iot.lighting_effect/set_lighting_effect")
            .expect("payload should contain set_lighting_effect");
        assert!(inner.get("id").is_some());
        assert!(inner.get("segments").is_some());
        assert!(inner.get("hue_range").is_some());
        assert!(inner.get("init_states").is_some());
    }

    #[test]
    fn lightstrip_state_uses_lightstrip_namespace_and_method() {
        let payload = lightstrip_state_payload(json!({"brightness": 42, "on_off": 1}));
        assert_eq!(
            payload.pointer("/smartlife.iot.lightStrip/set_light_state/brightness"),
            Some(&json!(42))
        );
        assert!(
            payload
                .pointer("/smartlife.iot.smartbulb.lightingservice")
                .is_none()
        );
    }
}
