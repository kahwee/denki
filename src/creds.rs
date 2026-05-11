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

pub fn load() -> Result<(String, String)> {
    if let (Ok(user), Ok(pass)) = (std::env::var("TAPO_USER"), std::env::var("TAPO_PASS")) {
        return Ok((user, pass));
    }

    let path = creds_path();
    let data = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "No Tapo credentials found.\n  Set TAPO_USER and TAPO_PASS, or run: denki login <email> <password>\n  (saves to {})",
            path.display()
        )
    })?;

    let creds: CredFile = serde_json::from_str(&data).context(
        "credentials.json is malformed — run `denki login <email> <password>` to reset it",
    )?;

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
