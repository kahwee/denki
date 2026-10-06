use super::energy::EnergyReader;
use crate::cli::WatchFormat;
use anyhow::Result;
use serde_json::{Value, json};
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub async fn handle_energy_watch(
    host: &str,
    outlet: Option<u8>,
    interval: u64,
    count: Option<u64>,
    mut format: WatchFormat,
) -> Result<()> {
    if crate::output::is_json() {
        if format == WatchFormat::Csv {
            return Err(crate::error::error(
                "invalid_arguments",
                "Use either --json or --format csv",
            ));
        }
        format = WatchFormat::Jsonl;
    }
    let mut stdout = std::io::stdout();
    if format == WatchFormat::Csv {
        writeln!(
            stdout,
            "timestamp_unix_ms,device,outlet,status,power_w,voltage_v,current_a,energy_wh,today_energy_wh,month_energy_wh,error_code,error_message"
        )?;
        stdout.flush()?;
    }
    let mut reader: Option<EnergyReader> = None;
    let mut samples = 0u64;
    let mut failures = 0u64;
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);
    loop {
        let sample = async {
            if reader.is_none() {
                reader = Some(EnergyReader::connect(host, outlet).await?);
            }
            reader.as_mut().expect("reader initialized").sample().await
        };
        let result = tokio::select! {
            result = sample => result,
            signal = &mut signal => { signal?; break; }
        };
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let row = match result {
            Ok(measurement) => {
                json!({"schema_version":1,"timestamp_unix_ms":timestamp,"device":host,"outlet":outlet,"status":"ok","source":"device","measurement":measurement,"error":null})
            }
            Err(error) => {
                failures += 1;
                reader = None; // reconnect next interval; never fabricate a zero sample
                json!({"schema_version":1,"timestamp_unix_ms":timestamp,"device":host,"outlet":outlet,"status":"error","source":"device","measurement":null,"error":crate::output::failure(&error)})
            }
        };
        match format {
            WatchFormat::Jsonl => writeln!(stdout, "{row}")?,
            WatchFormat::Csv => writeln!(stdout, "{}", csv_row(&row))?,
            WatchFormat::Table => {
                if row["status"] == "ok" {
                    let power = row["measurement"]["power_w"]
                        .as_f64()
                        .map(|p| format!("{p:.3} W"))
                        .unwrap_or_else(|| "unavailable".into());
                    writeln!(stdout, "{timestamp}  {host}  {power}")?;
                } else {
                    writeln!(
                        stdout,
                        "{timestamp}  {host}  ERROR {}",
                        row["error"]["message"].as_str().unwrap_or("unknown")
                    )?;
                }
            }
        }
        stdout.flush()?;
        samples += 1;
        if count.is_some_and(|count| samples >= count) {
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(interval)) => {},
            signal = &mut signal => { signal?; break; }
        }
    }
    eprintln!("Energy watch: {samples} samples, {failures} failed.");
    if failures > 0 {
        return Err(crate::error::error(
            "partial_failure",
            format!("{failures} energy samples failed"),
        ));
    }
    Ok(())
}
fn csv_field(value: &Value) -> String {
    if value.is_null() {
        return String::new();
    }
    let text = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}
fn csv_row(row: &Value) -> String {
    let mut fields = vec![
        &row["timestamp_unix_ms"],
        &row["device"],
        &row["outlet"],
        &row["status"],
    ];
    for key in [
        "power_w",
        "voltage_v",
        "current_a",
        "energy_wh",
        "today_energy_wh",
        "month_energy_wh",
    ] {
        fields.push(&row["measurement"][key]);
    }
    fields.extend([&row["error"]["code"], &row["error"]["message"]]);
    fields
        .into_iter()
        .map(csv_field)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_quotes_aliases_and_multiline_errors_and_keeps_missing_samples_empty() {
        let row = json!({"timestamp_unix_ms":123,"device":"desk, \"A\"","outlet":null,"status":"error","measurement":null,"error":{"code":"io_error","message":"first\nsecond"}});
        assert_eq!(
            csv_row(&row),
            "123,\"desk, \"\"A\"\"\",,error,,,,,,,io_error,\"first\nsecond\""
        );
    }
}
