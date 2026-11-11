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
/// Whether a value can be rendered literally on one terminal line.
/// Includes Unicode direction/format controls which can conceal or reorder text.
pub fn is_terminal_safe(value: &str) -> bool {
    !value.chars().any(terminal_control)
}

fn terminal_control(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}' | '\u{feff}')
}

/// Escape untrusted terminal text before adding trusted color or layout.
/// Protocol values, stored aliases, and JSON output remain unchanged.
pub fn terminal_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        if terminal_control(c) {
            escaped.extend(c.escape_default());
        } else {
            escaped.push(c);
        }
    }
    escaped
}

/// Render machine JSON without emitting terminal-active Unicode controls.
/// JSON escapes preserve the exact values after deserialization.
pub fn json_text(value: &Value) -> String {
    let serialized = value.to_string();
    let mut escaped = String::with_capacity(serialized.len());
    for c in serialized.chars() {
        if terminal_control(c) {
            // Use JSON's UTF-16 escape spelling, including surrogate pairs.
            for unit in c.encode_utf16(&mut [0; 2]) {
                use std::fmt::Write;
                let _ = write!(escaped, "\\u{unit:04x}");
            }
        } else {
            escaped.push(c);
        }
    }
    escaped
}

macro_rules! println {
    ($($arg:tt)*) => { if $crate::output::is_json() { eprintln!($($arg)*); } else { std::println!($($arg)*); } };
}
pub(crate) use println;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_text_escapes_controls_and_preserves_visible_text() {
        for c in (0..=0x9f)
            .filter_map(char::from_u32)
            .filter(|c| c.is_control())
        {
            let raw = c.to_string();
            assert!(!is_terminal_safe(&raw));
            let escaped = terminal_text(&raw);
            assert!(is_terminal_safe(&escaped));
            assert_ne!(raw, escaped);
        }
        for c in [
            '\u{061c}', '\u{200b}', '\u{200d}', '\u{2028}', '\u{202e}', '\u{2066}', '\u{feff}',
        ] {
            assert!(!is_terminal_safe(&c.to_string()));
            assert!(is_terminal_safe(&terminal_text(&c.to_string())));
        }
        let visible = "Desk lamp 電気 café 🐸 $HOME `echo` ' \" \\";
        assert_eq!(terminal_text(visible), visible);
        assert_eq!(terminal_text("\r\n\t"), "\\r\\n\\t");
    }

    #[test]
    fn json_terminal_escaping_preserves_exact_machine_values() {
        let raw = r"x\u009b\u202e\u2066\u061c\u{e0001}";
        let raw = format!("{raw}\u{9b}\u{202e}\u{2066}\u{061c}\r\n\u{1b} 🐸");
        let value = json!({"alias":raw,"nested":[raw], raw.clone():raw});
        let rendered = json_text(&value);
        assert!(is_terminal_safe(&rendered));
        assert!(rendered.contains("\\u009b"));
        assert!(rendered.contains("\\u202e"));
        assert_eq!(serde_json::from_str::<Value>(&rendered).unwrap(), value);
    }

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
