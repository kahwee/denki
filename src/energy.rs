//! Normalized device-reported measurements shared by text, JSON, and watch.
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize, Default, PartialEq)]
pub struct Measurement {
    pub power_w: Option<f64>,
    pub voltage_v: Option<f64>,
    pub current_a: Option<f64>,
    pub energy_wh: Option<f64>,
    pub today_energy_wh: Option<f64>,
    pub month_energy_wh: Option<f64>,
}
fn number(data: &Value, key: &str, scale: f64) -> Result<Option<f64>> {
    match data.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && (n * scale).is_finite())
            .map(|n| Some(n * scale))
            .ok_or_else(|| crate::error::malformed(format!("Invalid energy field {key}"))),
    }
}
fn units(data: &Value, base: &str, scaled: &str, factor: f64) -> Result<Option<f64>> {
    let scaled = number(data, scaled, factor)?;
    let base = number(data, base, 1.0)?;
    Ok(scaled.or(base))
}
impl Measurement {
    pub fn kasa(response: &Value) -> Result<Self> {
        let data = response
            .pointer("/emeter/get_realtime")
            .or_else(|| response.pointer("/smartlife.iot.common.emeter/get_realtime"))
            .ok_or_else(|| crate::error::malformed("Missing energy measurements"))?;
        let measurement = Self {
            power_w: units(data, "power", "power_mw", 0.001)?,
            voltage_v: units(data, "voltage", "voltage_mv", 0.001)?,
            current_a: units(data, "current", "current_ma", 0.001)?,
            energy_wh: number(data, "total_wh", 1.0)?.or(number(data, "total", 1000.0)?),
            ..Self::default()
        };
        measurement.validate()?;
        Ok(measurement)
    }
    pub fn tapo(response: &Value) -> Result<Self> {
        let data = response
            .get("result")
            .ok_or_else(|| crate::error::malformed("Missing Tapo energy result"))?;
        let measurement = Self {
            power_w: number(data, "current_power", 0.001)?,
            today_energy_wh: number(data, "today_energy", 1.0)?,
            month_energy_wh: number(data, "month_energy", 1.0)?,
            ..Self::default()
        };
        measurement.validate()?;
        Ok(measurement)
    }
    fn validate(&self) -> Result<()> {
        if [
            self.power_w,
            self.energy_wh,
            self.today_energy_wh,
            self.month_energy_wh,
        ]
        .iter()
        .all(Option::is_none)
        {
            return Err(crate::error::malformed(
                "Device returned no power or energy measurements",
            ));
        }
        Ok(())
    }
    pub fn print(&self) {
        for (label, value, unit) in [
            ("Power", self.power_w, "W"),
            ("Voltage", self.voltage_v, "V"),
            ("Current", self.current_a, "A"),
            ("Total", self.energy_wh, "Wh"),
            ("Today", self.today_energy_wh, "Wh"),
            ("This month", self.month_energy_wh, "Wh"),
        ] {
            if let Some(value) = value {
                crate::output::println!("{label:12} {value:.3} {unit}");
            }
        }
    }
}

pub fn history(response: &Value, monthly: bool) -> Result<Vec<Value>> {
    let (method, list, key, max) = if monthly {
        ("get_monthstat", "month_list", "month", 12)
    } else {
        ("get_daystat", "day_list", "day", 31)
    };
    let data = response
        .pointer(&format!("/emeter/{method}/{list}"))
        .or_else(|| response.pointer(&format!("/smartlife.iot.common.emeter/{method}/{list}")))
        .and_then(Value::as_array)
        .ok_or_else(|| crate::error::malformed("Missing energy history list"))?;
    let mut rows = Vec::new();
    for entry in data {
        let period = entry
            .get(key)
            .and_then(Value::as_u64)
            .filter(|n| (1..=max).contains(n))
            .ok_or_else(|| crate::error::malformed("Invalid energy history period"))?;
        let wh = number(entry, "energy_wh", 1.0)?
            .or(number(entry, "energy", 1000.0)?)
            .ok_or_else(|| crate::error::malformed("Missing history energy measurement"))?;
        rows.push(serde_json::json!({key: period, "energy_wh": wh}));
    }
    rows.sort_by_key(|row| row[key].as_u64());
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn model_units_normalize_without_inventing_missing_fields() {
        let old=Measurement::kasa(&json!({"emeter":{"get_realtime":{"power":1.25,"voltage":230.0,"current":0.02,"total":0.005}}})).unwrap();
        let new=Measurement::kasa(&json!({"emeter":{"get_realtime":{"power_mw":1250,"voltage_mv":230000,"current_ma":20,"total_wh":5}}})).unwrap();
        assert_eq!(old, new);
        let bulb = Measurement::kasa(
            &json!({"smartlife.iot.common.emeter":{"get_realtime":{"power_mw":0,"total_wh":3}}}),
        )
        .unwrap();
        assert_eq!(bulb.power_w, Some(0.0));
        assert!(bulb.voltage_v.is_none());
        let tapo = Measurement::tapo(
            &json!({"result":{"current_power":1250,"today_energy":3,"month_energy":9}}),
        )
        .unwrap();
        assert_eq!(tapo.power_w, Some(1.25));
        assert_eq!(tapo.today_energy_wh, Some(3.0));
        assert!(tapo.energy_wh.is_none());
    }
    #[test]
    fn invalid_or_absent_measurements_are_not_zero() {
        for value in [
            json!({}),
            json!({"power":-1}),
            json!({"power":"1"}),
            json!({"power":null}),
            json!({"voltage":230}),
            json!({"total":1e308}),
        ] {
            assert!(Measurement::kasa(&json!({"emeter":{"get_realtime":value}})).is_err());
        }
        assert!(Measurement::tapo(&json!({"result":{}})).is_err());
        assert!(Measurement::tapo(&json!({"result":{"current_power":"1"}})).is_err());
    }
    #[test]
    fn history_distinguishes_empty_history_from_malformed_history() {
        assert!(
            history(&json!({"emeter":{"get_daystat":{"day_list":[]}}}), false)
                .unwrap()
                .is_empty()
        );
        for value in [
            json!({}),
            json!({"emeter":{"get_daystat":{}}}),
            json!({"emeter":{"get_daystat":{"day_list":[{"day":1}]}}}),
            json!({"emeter":{"get_daystat":{"day_list":[{"day":0,"energy_wh":4}]}}}),
        ] {
            assert!(history(&value, false).is_err());
        }
    }
}
