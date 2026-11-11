//! Pure diagnostic interpretation; no device I/O or terminal rendering.
use crate::{devices, hosts};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct DoctorReport {
    pub(super) schema_version: u8,
    pub(super) status: &'static str,
    pub(super) alias: Option<String>,
    pub(super) ip: String,
    pub(super) protocol: String,
    pub(super) reachable: bool,
    pub(super) physically_confirmed: bool,
    pub(super) model: Option<String>,
    pub(super) kind: Option<String>,
    pub(super) hardware_version: Option<String>,
    pub(super) firmware_version: Option<String>,
    pub(super) signal_dbm: Option<i64>,
    pub(super) power_on: Option<bool>,
    pub(super) capabilities: Vec<String>,
    pub(super) verified_model: Option<bool>,
    pub(super) warnings: Vec<String>,
    pub(super) next_steps: Vec<String>,
    pub(super) error: Option<String>,
    pub(super) error_code: Option<&'static str>,
}

impl DoctorReport {
    pub(super) fn pending(alias: Option<String>, ip: String, protocol: &hosts::Protocol) -> Self {
        Self {
            schema_version: 1,
            status: "error",
            alias,
            ip,
            protocol: protocol.to_string(),
            reachable: false,
            physically_confirmed: false,
            model: None,
            kind: None,
            hardware_version: None,
            firmware_version: None,
            signal_dbm: None,
            power_on: None,
            capabilities: Vec::new(),
            verified_model: None,
            warnings: Vec::new(),
            next_steps: Vec::new(),
            error: None,
            error_code: None,
        }
    }

    pub(super) fn apply_registry(&mut self) {
        let Some(model) = self.model.as_deref() else {
            return;
        };
        if let Some(entry) = devices::lookup(model) {
            if self.protocol == "tapo" {
                self.capabilities = entry.tapo_auto_supports.clone();
                self.verified_model = Some(entry.tapo_auto_verified);
                self.warnings.push("Tapo auto mode negotiates TPAP/KLAP; upstream does not expose the selected wire protocol.".into());
                if !entry.tapo_auto_verified {
                    self.warnings.push("Full auto-adapter support is not hardware-verified; see the support guide for tested operations.".into());
                }
                return;
            }
            self.capabilities = entry.supports.clone();
            self.verified_model = Some(entry.verified);
            if !entry.verified {
                self.warnings
                    .push("This model is supported but not verified on physical hardware.".into());
            }
        } else {
            self.warnings.push(format!(
                "Model {model} is not present in the denki capability registry."
            ));
        }
    }
}

impl DoctorReport {
    /// Preserve connectivity already established by a successful handshake.
    pub(super) fn record_failure(&mut self, error: &anyhow::Error) {
        let code = crate::error::code(error);
        self.status = "error";
        self.reachable |= matches!(
            code,
            "malformed_response"
                | "device_rejected"
                | "identity_mismatch"
                | "authentication_failed"
                | "tpap_credentials"
                | "tpap_auth_attempts_limit"
                | "unsupported_protocol"
        );
        self.error_code = Some(code);
        self.error = Some(format!("{error:#}"));
    }

    pub(super) fn add_guidance(&mut self) {
        if self.signal_dbm.is_some_and(|signal| signal < -70) {
            self.warnings
                .push("Weak Wi-Fi signal can cause intermittent device failures.".into());
            self.next_steps.push(
                "Check signal in the vendor app and improve Wi-Fi coverage at the device.".into(),
            );
        }
        let step = match self.error_code {
            Some("connection_failed" | "timeout") => Some(
                "Check device power and its current IP in the vendor app or router. A saved alias alone does not prove the device is still at that address.",
            ),
            Some("authentication_failed" | "tpap_credentials") => Some(
                "Verify the account credentials with `denki login EMAIL` and check Third-Party Compatibility in the vendor app. Do not reset or remove the device just because authentication failed.",
            ),
            Some("tpap_auth_attempts_limit") => Some(
                "Stop login retries and wait for the device's authentication lockout to expire before checking credentials.",
            ),
            Some("unsupported_protocol") => Some(
                "Check Third-Party Compatibility in the vendor app and Denki's supported protocols. Updating credentials alone will not fix an unsupported protocol.",
            ),
            Some("authentication_policy") => Some(
                "This device cannot use the required authentication protocol. Review DENKI_REQUIRE_TPAP and the device's protocol before changing your policy.",
            ),
            Some("alias_conflict") => Some(
                "Use `denki aliases --json` to review aliases sharing this address. Verify the physical device, then correct its address or protocol with `denki alias NAME IP` (and --tapo or --klap when appropriate), or remove a stale alias with `denki unalias NAME`. Rerun `denki doctor --all` after resolving the conflict.",
            ),
            Some("identity_mismatch") => Some(
                "Verify the physical device and its current address before updating the saved alias. Do not send power commands to a mismatched device.",
            ),
            _ => None,
        };
        if let Some(step) = step {
            self.next_steps.push(step.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_preserves_handshake_evidence_and_recognizes_device_replies() {
        let mut report = DoctorReport::pending(None, "192.0.2.1".into(), &hosts::Protocol::Klap);
        report.record_failure(&crate::error::error("connection_failed", "refused"));
        assert!(!report.reachable);
        report.record_failure(&crate::error::error("authentication_failed", "rejected"));
        assert!(report.reachable);
        report.record_failure(&crate::error::error("timeout", "read timed out"));
        assert!(report.reachable);
        assert_eq!(report.error_code, Some("timeout"));
        let mut unsupported =
            DoctorReport::pending(None, "192.0.2.2".into(), &hosts::Protocol::Tapo);
        unsupported.record_failure(&crate::error::error(
            "unsupported_protocol",
            "unsupported reply",
        ));
        assert!(unsupported.reachable);
    }

    #[test]
    fn guidance_distinguishes_authentication_protocol_and_lockout() {
        for (code, expected) in [
            ("authentication_failed", "denki login"),
            ("unsupported_protocol", "credentials alone will not fix"),
            ("tpap_auth_attempts_limit", "Stop login retries"),
            ("identity_mismatch", "Verify the physical device"),
            ("connection_failed", "saved alias alone"),
            ("timeout", "current IP"),
            ("alias_conflict", "denki aliases --json"),
        ] {
            let mut report =
                DoctorReport::pending(None, "192.0.2.1".into(), &hosts::Protocol::Tapo);
            report.error_code = Some(code);
            report.add_guidance();
            assert!(
                report.next_steps.iter().any(|step| step.contains(expected)),
                "{code}"
            );
        }
    }

    #[test]
    fn weak_signal_advice_does_not_mark_a_working_device_failed() {
        let mut report = DoctorReport::pending(None, "192.0.2.1".into(), &hosts::Protocol::Tapo);
        report.status = "ok";
        report.reachable = true;
        report.signal_dbm = Some(-71);
        report.add_guidance();
        assert_eq!(report.status, "ok");
        assert!(report.error.is_none());
        assert_eq!(report.next_steps.len(), 1);
        assert!(report.warnings[0].contains("Weak Wi-Fi"));
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["signal_dbm"], -71);
        assert_eq!(json["next_steps"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn missing_or_adequate_signal_does_not_invent_a_wifi_problem() {
        for signal in [None, Some(-70), Some(-40)] {
            let mut report =
                DoctorReport::pending(None, "192.0.2.1".into(), &hosts::Protocol::Kasa);
            report.signal_dbm = signal;
            report.add_guidance();
            assert!(report.next_steps.is_empty());
            assert!(report.warnings.is_empty());
        }
    }
}
