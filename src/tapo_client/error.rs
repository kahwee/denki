use ::tapo::{Error, TapoResponseError};

// Do not include upstream free-form descriptions or raw error chains: they can
// contain response payloads, network names, or authentication material.
pub(super) fn upstream_error(stage: &str, error: Error) -> anyhow::Error {
    if let Error::Other(ref inner) = error
        && matches!(
            crate::error::code(inner),
            "identity_mismatch" | "malformed_response"
        )
    {
        return crate::error::error(crate::error::code(inner), format!("Tapo {stage}: {inner}"));
    }
    let code = match &error {
        Error::Tapo(TapoResponseError::Unauthorized {
            kind: "TPAP_CREDENTIALS",
            ..
        }) => "tpap_credentials",
        Error::Tapo(TapoResponseError::Unauthorized {
            kind: "TPAP_AUTH_ATTEMPTS_LIMIT",
            ..
        }) => "tpap_auth_attempts_limit",
        Error::Tapo(TapoResponseError::Unauthorized { .. }) => "authentication_failed",
        Error::Serde(_) => "malformed_response",
        Error::UnsupportedProtocol { .. } => "unsupported_protocol",
        Error::Http(error) if error.is_timeout() => "timeout",
        Error::Http(_) => "connection_failed",
        Error::Tapo(TapoResponseError::DeviceError { .. }) => "device_rejected",
        _ => "command_failed",
    };
    let detail = match error {
        Error::Tapo(TapoResponseError::Unauthorized { kind: "TPAP_CREDENTIALS", .. }) =>
            "TPAP_CREDENTIALS: credentials rejected. Check TAPO_USER/TAPO_PASS or run `denki login <email>`. Do not retry in a loop.".to_owned(),
        Error::Tapo(TapoResponseError::Unauthorized { kind: "TPAP_AUTH_ATTEMPTS_LIMIT", .. }) =>
            "TPAP_AUTH_ATTEMPTS_LIMIT: device login is locked. Stop retrying and wait before another attempt.".to_owned(),
        Error::Tapo(TapoResponseError::Unauthorized { .. }) =>
            "authentication or session verification failed; check account credentials and the device's compatibility setting".to_owned(),
        Error::Serde(_) => "could not parse the upstream device response".to_owned(),
        Error::UnsupportedProtocol { .. } => "device advertised an unsupported protocol variant".to_owned(),
        Error::Http(error) if error.is_timeout() => "network request timed out".to_owned(),
        Error::Http(_) => "HTTP connection or response failed; check local network reachability".to_owned(),
        Error::Tapo(TapoResponseError::DeviceError { code, .. }) => format!("device rejected the request (code {code})"),
        _ => "upstream request failed".to_owned(),
    };
    crate::error::error(code, format!("Tapo {stage}: {detail}"))
}
