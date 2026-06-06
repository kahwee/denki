//! Tapo credentials — env vars (TAPO_USER/TAPO_PASS) take precedence over
//! ~/.config/denki/credentials.json (written mode 0600 on Unix).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct CredFile {
    tapo_user: String,
    tapo_pass: String,
}

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
    if let (Ok(user), Ok(pass)) = (std::env::var("TAPO_USER"), std::env::var("TAPO_PASS")) {
        return Ok((user, pass));
    }

    let path = creds_path();
    let data = std::fs::read_to_string(&path).with_context(|| missing_creds_context(&path))?;

    let creds: CredFile = serde_json::from_str(&data).context(malformed_creds_context())?;

    Ok((creds.tapo_user, creds.tapo_pass))
}

pub fn save(user: &str, pass: &str) -> Result<()> {
    let path = creds_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    std::fs::write(
        &path,
        serde_json::to_string_pretty(&CredFile {
            tapo_user: user.to_string(),
            tapo_pass: pass.to_string(),
        })?,
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

pub fn path_display() -> String {
    creds_path().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
