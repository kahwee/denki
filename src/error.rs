//! Stable error categories shared by the library and CLI.
use std::fmt;
#[derive(Debug)]
pub struct DeviceError {
    pub code: &'static str,
    pub message: String,
}
impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for DeviceError {}
pub fn error(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    DeviceError {
        code,
        message: message.into(),
    }
    .into()
}
pub fn malformed(message: impl Into<String>) -> anyhow::Error {
    error("malformed_response", message)
}
pub fn code(error: &anyhow::Error) -> &'static str {
    if let Some(e) = error.downcast_ref::<DeviceError>() {
        e.code
    } else if error.downcast_ref::<std::io::Error>().is_some() {
        "io_error"
    } else if error.downcast_ref::<serde_json::Error>().is_some() {
        "malformed_response"
    } else {
        "command_failed"
    }
}
