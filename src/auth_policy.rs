//! Local opt-in protocol policy. The setting is read before non-TPAP handshakes,
//! including session refreshes; no credentials are sent on a rejected path.
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

fn parse(value: Option<&str>) -> Result<bool> {
    match value {
        None | Some("0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        _ => Err(crate::error::error(
            "authentication_policy",
            "DENKI_REQUIRE_TPAP must be 1, true, 0 or false",
        )),
    }
}

pub(crate) fn non_tpap(klap: bool) -> Result<()> {
    let value = std::env::var("DENKI_REQUIRE_TPAP");
    let required = match &value {
        Ok(value) => parse(Some(value))?,
        Err(std::env::VarError::NotPresent) => false,
        Err(_) => {
            return Err(crate::error::error(
                "authentication_policy",
                "DENKI_REQUIRE_TPAP must be valid text",
            ));
        }
    };
    if required {
        return Err(crate::error::error(
            "authentication_policy",
            "DENKI_REQUIRE_TPAP blocks this non-TPAP login before authentication. Use a TPAP-capable device and a --tapo alias; KLAP-only devices are not compatible with this policy.",
        ));
    }
    if klap {
        static WARNED: AtomicBool = AtomicBool::new(false);
        if !WARNED.swap(true, Ordering::Relaxed) {
            eprintln!(
                "Warning: KLAP handshake captures permit offline password guessing. Use a strong unique Tapo password and trusted LAN, or DENKI_REQUIRE_TPAP=1 to reject KLAP fallback."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_protocol_policy_fails_closed() {
        assert!(parse(Some("1")).unwrap());
        assert!(parse(Some("true")).unwrap());
        assert!(!parse(None).unwrap());
        assert!(!parse(Some("false")).unwrap());
        for value in ["", "yes", "TRUE", "sentinel"] {
            let error = parse(Some(value)).unwrap_err();
            assert_eq!(crate::error::code(&error), "authentication_policy");
            assert!(!error.to_string().contains("sentinel"));
        }
    }
}
