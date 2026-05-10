//! Device operations — one function per API call, split by device type.
//!
//! All functions take a host IP string and return anyhow::Result.
//! Callers in main.rs are responsible for detecting device type first
//! and routing to the correct function (bulb_* vs plug_*).
//!
//! ── Bulb (KL135) API namespace ──────────────────────────────────────────────
//! All lighting commands go through:
//!   smartlife.iot.smartbulb.lightingservice / transition_light_state
//!
//! Energy commands use a different namespace than standard plugs:
//!   smartlife.iot.common.emeter  (NOT the bare "emeter" module)
//!   The bare "emeter" returns -2001 on KL135 — a common gotcha.
//!
//! ── Plug (KP115) API namespace ───────────────────────────────────────────────
//! Power: system / set_relay_state  (not lightingservice)
//! Energy: emeter  (standard module, returns V + A + W + Wh)
//! Schedule: schedule / get_rules
//! Time: time / get_time
//! LED: system / set_led_off  (note: 1 = LED disabled, 0 = LED enabled — inverted)
//!
//! ── Shared commands ──────────────────────────────────────────────────────────
//! Rename: system / set_dev_alias
//! Reboot: system / reboot

use crate::klap::KlapSession;
use crate::transport;
use anyhow::Result;
use serde_json::json;

// ── Universal ─────────────────────────────────────────────────────────────────

/// Fetch the full system info blob. Works on all device types.
/// Used to detect device type (mic_type) before issuing typed commands.
pub async fn sysinfo(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"system": {"get_sysinfo": {}}})).await
}

// ── Bulb (KL135) — on/off ────────────────────────────────────────────────────

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

/// Turn the bulb on. Uses transition_period: 0 for instant change.
/// The lightingservice namespace is KL135-specific — do not use on plugs.
pub async fn bulb_on(host: &str) -> Result<()> { bulb_set_power(host, true).await }

/// Turn the bulb off.
pub async fn bulb_off(host: &str) -> Result<()> { bulb_set_power(host, false).await }

/// Toggle the bulb. Reads current state first, then flips it.
/// Returns true if the bulb ended up on, false if it ended up off.
///
/// **Note:** this makes two round trips (sysinfo + set). The CLI avoids this
/// by reusing an already-fetched sysinfo blob via `kasa_exec_power`. Prefer
/// that pattern when you already have sysinfo in hand.
pub async fn bulb_toggle(host: &str) -> Result<bool> {
    let info = sysinfo(host).await?;
    let on = info
        .pointer("/system/get_sysinfo/light_state/on_off")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if on == 1 {
        bulb_off(host).await?;
        Ok(false)
    } else {
        bulb_on(host).await?;
        Ok(true)
    }
}

// ── Bulb (KL135) — light settings ────────────────────────────────────────────

/// Set brightness 0–100. Does not change color or color temperature.
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

/// Set color temperature in Kelvin (2500–9000).
/// This puts the bulb into CCT mode. Setting hue/saturation to 0 is required
/// to clear any previous color mode state on the device.
pub async fn bulb_set_warmth(host: &str, kelvin: u16) -> Result<()> {
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

/// Set HSV color (hue 0–360, saturation 0–100, value/brightness 0–100).
///
/// Setting saturation > 0 activates color mode and disables CCT mode.
/// color_temp must be set to 0 explicitly, otherwise the device may
/// ignore the hue/saturation values on some firmware versions.
pub async fn bulb_set_color(host: &str, hue: u16, saturation: u8, value: u8) -> Result<()> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {
            "transition_light_state": {
                "hue": hue,
                "saturation": saturation,
                "brightness": value,
                "color_temp": 0, // must be 0 to activate color mode
                "transition_period": 0
            }
        }}),
    )
    .await?;
    Ok(())
}

// ── Bulb (KL135) — info ───────────────────────────────────────────────────────

/// Fetch hardware specs: beam angle, wattage, lumens, CRI, voltage range.
/// Response path: /smartlife.iot.smartbulb.lightingservice/get_light_details
///
/// KL135 values: 220° beam, 10W, 60W equivalent, 800lm, CRI 90, 100–120V
pub async fn bulb_specs(host: &str) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {"get_light_details": {}}}),
    )
    .await
}

/// Fetch the 4 saved light presets (index 0–3).
/// Response path: /smartlife.iot.smartbulb.lightingservice/get_preferred_state/states
///
/// Default presets: warm white 2700K 50%, red 100%, green 100%, blue 100%
pub async fn bulb_presets(host: &str) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.smartbulb.lightingservice": {"get_preferred_state": {}}}),
    )
    .await
}

// ── Bulb (KL135) — energy ─────────────────────────────────────────────────────
//
// IMPORTANT: KL135 uses "smartlife.iot.common.emeter", NOT "emeter".
// Using the bare "emeter" module returns: {"err_code": -2001, "err_msg": "module not support"}
//
// KL135 energy response is also narrower than KP115:
//   - power_mw: current draw in milliwatts  ✓
//   - total_wh: lifetime total in watt-hours ✓
//   - voltage_mv: NOT present (bulb has no voltage sensor)
//   - current_ma: NOT present (bulb has no current sensor)

/// Real-time power draw from the bulb.
/// Returns power_mw and total_wh only (no voltage or current).
pub async fn bulb_energy(host: &str) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.common.emeter": {"get_realtime": {}}}),
    )
    .await
}

/// Daily energy usage for a specific month.
/// Response path: /smartlife.iot.common.emeter/get_daystat/day_list
/// Each entry: {year, month, day, energy_wh}. Days with 0 usage are omitted.
pub async fn bulb_energy_daily(host: &str, year: u16, month: u8) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.common.emeter": {
            "get_daystat": {"month": month, "year": year}
        }}),
    )
    .await
}

/// Monthly energy totals for a full year.
/// Response path: /smartlife.iot.common.emeter/get_monthstat/month_list
/// Each entry: {year, month, energy_wh}. Months with 0 usage are omitted.
pub async fn bulb_energy_monthly(host: &str, year: u16) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"smartlife.iot.common.emeter": {"get_monthstat": {"year": year}}}),
    )
    .await
}

// ── Dimmer (HS220) — brightness ──────────────────────────────────────────────
//
// HS220 uses a separate smartlife.iot.dimmer namespace for brightness control.
// The bulb lightingservice namespace does NOT work on dimmers.
//
// Important: brightness=0 is invalid on HS220 hardware. To turn off, use
// set_relay_state (same as a plain plug). So dimmer_set_brightness(host, 0)
// routes to plug_off instead of sending an invalid brightness command.

/// Set HS220 dimmer brightness 1–100.
/// Sending 0 is invalid; routes to plug_off instead.
pub async fn dimmer_set_brightness(host: &str, level: u8) -> Result<()> {
    if level == 0 {
        return plug_off(host).await;
    }
    transport::send(
        host,
        json!({"smartlife.iot.dimmer": {"set_brightness": {"brightness": level}}}),
    )
    .await?;
    Ok(())
}

// ── Plug (KP115) — on/off ────────────────────────────────────────────────────

/// Turn the plug's relay on. Uses set_relay_state (not lightingservice).
pub async fn plug_on(host: &str) -> Result<()> {
    transport::send(host, json!({"system": {"set_relay_state": {"state": 1}}})).await?;
    Ok(())
}

/// Turn the plug's relay off.
pub async fn plug_off(host: &str) -> Result<()> {
    transport::send(host, json!({"system": {"set_relay_state": {"state": 0}}})).await?;
    Ok(())
}

/// Toggle the plug. Reads relay_state first, then flips it.
/// Returns true if the relay ended up on, false if off.
///
/// **Note:** this makes two round trips (sysinfo + set). The CLI avoids this
/// by reusing an already-fetched sysinfo blob via `kasa_exec_power`. Prefer
/// that pattern when you already have sysinfo in hand.
pub async fn plug_toggle(host: &str) -> Result<bool> {
    let info = sysinfo(host).await?;
    // relay_state is in sysinfo root, not inside a light_state sub-object
    let on = info
        .pointer("/system/get_sysinfo/relay_state")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if on == 1 {
        plug_off(host).await?;
        Ok(false)
    } else {
        plug_on(host).await?;
        Ok(true)
    }
}

// ── Plug (KP115) — controls ───────────────────────────────────────────────────

/// Control the physical LED indicator on the plug body.
///
/// Note the inverted naming: the command is "set_led_off" and the field is "off".
///   off: 0 → LED is ON  (lit)
///   off: 1 → LED is OFF (dark)
pub async fn plug_led(host: &str, on: bool) -> Result<()> {
    transport::send(
        host,
        // When `on` is true we want LED lit, so off=0; when false, off=1
        json!({"system": {"set_led_off": {"off": if on { 0 } else { 1 }}}}),
    )
    .await?;
    Ok(())
}

// ── Plug (KP115) — energy ────────────────────────────────────────────────────
//
// KP115 uses the standard "emeter" module (unlike KL135 which uses smartlife namespace).
// Full response includes: current_ma, voltage_mv, power_mw, total_wh — all four fields.
// This is because the KP115 has a dedicated current sensing chip (ADE7953 or similar).

/// Real-time energy reading from the plug.
/// Returns current_ma, voltage_mv, power_mw, and total_wh.
pub async fn plug_energy(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"emeter": {"get_realtime": {}}})).await
}

/// Daily energy usage for a specific month.
/// Response path: /emeter/get_daystat/day_list
/// Days with no usage (plug was off all day) are omitted from the list.
pub async fn plug_energy_daily(host: &str, year: u16, month: u8) -> Result<serde_json::Value> {
    transport::send(
        host,
        json!({"emeter": {"get_daystat": {"month": month, "year": year}}}),
    )
    .await
}

/// Monthly energy totals for a full year.
/// Response path: /emeter/get_monthstat/month_list
pub async fn plug_energy_monthly(host: &str, year: u16) -> Result<serde_json::Value> {
    transport::send(host, json!({"emeter": {"get_monthstat": {"year": year}}})).await
}

// ── Plug (KP115) — info ───────────────────────────────────────────────────────

/// Fetch the list of schedule rules (on/off timers).
/// Response path: /schedule/get_rules/rule_list
/// Returns an empty list if no schedules have been configured.
pub async fn plug_schedules(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"schedule": {"get_rules": {}}})).await
}

/// Fetch the device's current local time.
/// Response path: /time/get_time — {year, month, mday, hour, min, sec}
///
/// Only supported on plugs. KL135 returns -2001 for time commands.
pub async fn plug_time(host: &str) -> Result<serde_json::Value> {
    transport::send(host, json!({"time": {"get_time": {}}})).await
}

// ── Strip (HS300/KP303) — per-outlet control ─────────────────────────────────
//
// Individual outlets are addressed via a `context.child_ids` wrapper.
// The child_id comes from sysinfo.children[i].id (callers must fetch sysinfo
// first to resolve the outlet number to the correct id).

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

/// Turn on one outlet on a power strip by its child id.
pub async fn strip_outlet_on(host: &str, child_id: &str) -> Result<()> {
    strip_outlet_set(host, child_id, true).await
}

/// Turn off one outlet on a power strip by its child id.
pub async fn strip_outlet_off(host: &str, child_id: &str) -> Result<()> {
    strip_outlet_set(host, child_id, false).await
}

// ── Strip (HS300/KP303) — per-outlet energy ──────────────────────────────────
//
// Per-outlet energy uses the same emeter namespace as plugs, but wrapped in
// context.child_ids to target a specific outlet.
// Strip-level energy (no context) works via the existing plug_energy* functions.

/// Real-time energy for one outlet on a power strip.
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

/// Daily energy usage for one outlet.
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

/// Monthly energy totals for one outlet.
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

/// Rename one outlet on a power strip.
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

// ── Shared — works on all device types ───────────────────────────────────────

/// Rename the device. Sets the alias shown in the Kasa app and in sysinfo.
pub async fn rename(host: &str, name: &str) -> Result<()> {
    transport::send(host, json!({"system": {"set_dev_alias": {"alias": name}}})).await?;
    Ok(())
}

/// Reboot the device after a 1-second delay.
/// The device will be unreachable for ~10–15 seconds while it restarts.
pub async fn restart(host: &str) -> Result<()> {
    transport::send(host, json!({"system": {"reboot": {"delay": 1}}})).await?;
    Ok(())
}

// ── Tapo / KLAP — works on P125 and other Tapo devices ───────────────────────

/// Fetch full device info from a Tapo device.
/// Method: get_device_info (no params needed)
pub async fn tapo_device_info(session: &mut KlapSession) -> Result<serde_json::Value> {
    session
        .send(&serde_json::to_string(&json!({"method": "get_device_info", "params": {}}))?)
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

/// Turn a Tapo device on.
pub async fn tapo_on(session: &mut KlapSession) -> Result<()> {
    tapo_set_power(session, true).await
}

/// Turn a Tapo device off.
pub async fn tapo_off(session: &mut KlapSession) -> Result<()> {
    tapo_set_power(session, false).await
}

/// Toggle a Tapo device. Returns true if now on, false if now off.
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

/// Check for a non-zero error_code in a Tapo response.
fn check_tapo_error(resp: &serde_json::Value) -> Result<()> {
    let code = resp.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code != 0 {
        anyhow::bail!("Tapo device error: code {code}");
    }
    Ok(())
}
