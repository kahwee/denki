//! Tapo credentials — env vars (TAPO_USER/TAPO_PASS) take precedence over
//! denki/credentials.json in the platform configuration directory (mode 0600 on Unix).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct CredFile {
    tapo_user: String,
    tapo_pass: String,
}

const MAX_CREDENTIAL_BYTES: u64 = 16 * 1024;

fn creds_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))
        .join("denki")
        .join("credentials.json")
}

fn missing_creds_context(path: &std::path::Path) -> String {
    format!(
        "No Tapo credentials found.\n\
         Set both TAPO_USER and TAPO_PASS, or run `denki login <email>` to save them.\n\
         File: {}",
        path.display()
    )
}

fn malformed_creds_context() -> &'static str {
    "credentials.json is malformed — remove it and run `denki login <email>` again"
}

pub fn load() -> Result<(String, String)> {
    match (std::env::var("TAPO_USER"), std::env::var("TAPO_PASS")) {
        (Ok(user), Ok(pass)) if !user.is_empty() && !pass.is_empty() => return Ok((user, pass)),
        (Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => (),
        _ => {
            return Err(crate::error::error(
                "invalid_credentials_environment",
                "Set both non-empty TAPO_USER and TAPO_PASS, or unset both to use saved credentials.",
            ));
        }
    }
    let path = creds_path();
    let data = crate::storage::read(&path, true, MAX_CREDENTIAL_BYTES)?
        .with_context(|| missing_creds_context(&path))?;

    let creds: CredFile = serde_json::from_str(&data)
        .map_err(|_| crate::error::error("malformed_configuration", malformed_creds_context()))?;
    if creds.tapo_user.is_empty() || creds.tapo_pass.is_empty() {
        return Err(crate::error::error(
            "malformed_configuration",
            malformed_creds_context(),
        ));
    }

    Ok((creds.tapo_user, creds.tapo_pass))
}

pub fn save(user: &str, pass: &str) -> Result<()> {
    let path = creds_path();
    if user.is_empty() || pass.is_empty() {
        return Err(crate::error::error(
            "invalid_credentials",
            "Email and password must both be non-empty",
        ));
    }
    let data = serde_json::to_vec_pretty(&CredFile {
        tapo_user: user.to_string(),
        tapo_pass: pass.to_string(),
    })?;
    if data.len() as u64 > MAX_CREDENTIAL_BYTES {
        return Err(crate::error::error(
            "configuration_too_large",
            "Credentials exceed the configuration size limit",
        ));
    }
    let _lock = crate::storage::FileLock::acquire(&path)?;
    crate::storage::atomic_write(&path, &data)?;

    Ok(())
}

pub fn path_display() -> String {
    creds_path().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_credentials_are_rejected_before_writing() {
        assert_eq!(
            crate::error::code(&save("", "synthetic-secret").unwrap_err()),
            "invalid_credentials"
        );
        assert_eq!(
            crate::error::code(&save("test@example.invalid", "").unwrap_err()),
            "invalid_credentials"
        );
        let error = save(
            "test@example.invalid",
            &"x".repeat(MAX_CREDENTIAL_BYTES as usize),
        )
        .unwrap_err();
        assert_eq!(crate::error::code(&error), "configuration_too_large");
        assert!(!error.to_string().contains("test@example.invalid"));
    }

    #[test]
    fn missing_creds_context_mentions_env_and_file_path() {
        let path = PathBuf::from("/tmp/denki/credentials.json");
        let msg = missing_creds_context(&path);
        assert!(msg.contains("TAPO_USER"));
        assert!(msg.contains("TAPO_PASS"));
        assert!(msg.contains("denki login <email>"));
        assert!(msg.contains("/tmp/denki/credentials.json"));
    }

    #[test]
    fn malformed_creds_message_mentions_reset_flow() {
        assert!(malformed_creds_context().contains("malformed"));
        assert!(malformed_creds_context().contains("remove it"));
        assert!(malformed_creds_context().contains("denki login <email>"));
    }
}
