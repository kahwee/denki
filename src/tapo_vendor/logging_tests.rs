//! Regression guard for the audited control-path logging expressions.
#[test]
fn control_logs_never_interpolate_payloads_or_identifiers() {
    let sources = [
        include_str!("api/api_client.rs"),
        include_str!("api/discovery/device_discovery.rs"),
        include_str!("api/discovery/device_discovery_raw.rs"),
        include_str!("api/protocol/tapo_protocol.rs"),
        include_str!("api/protocol/klap_protocol.rs"),
        include_str!("api/protocol/tpap_protocol.rs"),
        include_str!("api/protocol/aes_ssl_protocol.rs"),
    ];
    // An exact allowlist means a new interpolated field requires review. HTTP
    // statuses, numeric error codes and fixed login sub-methods are safe.
    let allowed = [
        "debug!(\"Response error: {}\", response.status());",
        "debug!(\"Handshake1 error: {}\", response.status());",
        "debug!(\"Handshake2 error: {}\", response.status());",
        "debug!(\"Handshake1 error code: {}\", response_body.error_code);",
        "debug!(\"Handshake2 error code: {}\", response_body.error_code);",
        "debug!(\"Discover error: {}\", response.status());",
        "debug!(\"Performing {sub_method}...\");",
        "debug!(\"{sub_method} error: {}\", response.status());",
        "debug!(\"{sub_method} error code: {}\", response_body.error_code);",
        "debug!(\"Device responded with error code {error_code} in plain text\");",
    ];
    for source in sources {
        for line in source.lines() {
            if ["debug!(", "trace!(", "warn!(", "info!(", "error!("]
                .iter()
                .any(|invocation| line.contains(invocation))
            {
                assert!(
                    line.contains(");") || line.ends_with("),"),
                    "Review multiline log: {line}"
                );
                if line.contains('{') {
                    assert!(
                        allowed.contains(&line.trim()),
                        "Unaudited dynamic log: {line}"
                    );
                }
            }
        }
    }
}
