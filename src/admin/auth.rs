use anyhow::Result;

use crate::creds;

pub fn handle_login(email: &str, password: Option<String>) -> Result<()> {
    let password = match password {
        Some(p) => p,
        None => rpassword::prompt_password("Tapo password: ")
            .map_err(|e| anyhow::anyhow!("Failed to read password: {e}"))?,
    };
    creds::save(email, &password)?;
    println!("Tapo credentials saved to {}", creds::path_display());
    println!("(File is readable only by you. Use TAPO_USER/TAPO_PASS env vars to override.)");
    Ok(())
}
