//! CLI result collection. Task-local state keeps concurrent library callers isolated.
use serde_json::{Value, json};
use std::cell::RefCell;
tokio::task_local! { static OUTPUT: RefCell<Output>; }
struct Output {
    json: bool,
    data: Value,
}
pub fn is_json() -> bool {
    OUTPUT.try_with(|o| o.borrow().json).unwrap_or(false)
}
pub fn record(data: Value) {
    let _ = OUTPUT.try_with(|o| o.borrow_mut().data = data);
}
pub async fn collect<F: std::future::Future>(json: bool, future: F) -> (F::Output, Value) {
    OUTPUT
        .scope(
            RefCell::new(Output {
                json,
                data: json!({}),
            }),
            async {
                let result = future.await;
                let data = OUTPUT.with(|o| o.borrow().data.clone());
                (result, data)
            },
        )
        .await
}
pub fn failure(error: &anyhow::Error) -> Value {
    json!({"code": crate::error::code(error), "message": format!("{error:#}")})
}
pub fn envelope(command: &str, result: &anyhow::Result<()>, data: Value) -> Value {
    json!({"schema_version": 2, "command": command,
        "status": if result.is_ok() { "ok" } else { "error" }, "data": data,
        "error": result.as_ref().err().map(failure)})
}
/// Remove local identifiers from user-facing diagnostics and API-derived output.
pub fn sanitized(mut data: Value) -> Value {
    match &mut data {
        Value::Object(map) => {
            map.retain(|key, _| {
                !matches!(
                    key.as_str(),
                    "id" | "deviceId"
                        | "device_id"
                        | "mac"
                        | "mic_mac"
                        | "ssid"
                        | "hwId"
                        | "oemId"
                        | "fwId"
                )
            });
            for value in map.values_mut() {
                *value = sanitized(value.take());
            }
        }
        Value::Array(values) => {
            for value in values {
                *value = sanitized(value.take());
            }
        }
        _ => {}
    }
    data
}
macro_rules! println {
    ($($arg:tt)*) => { if $crate::output::is_json() { eprintln!($($arg)*); } else { std::println!($($arg)*); } };
}
pub(crate) use println;

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn concurrent_result_contexts_are_isolated() {
        let collect_one = |n| {
            collect(true, async move {
                record(json!({"value":n}));
                tokio::task::yield_now().await;
            })
        };
        let (a, b) = tokio::join!(collect_one(1), collect_one(2));
        assert_eq!(a.1, json!({"value":1}));
        assert_eq!(b.1, json!({"value":2}));
        assert!(!is_json());
    }
}
