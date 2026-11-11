use crate::output::terminal_text;
use anyhow::Result;

use crate::creds;

const MAX_PASSWORD_BYTES: u64 = 4096;

fn read_password(reader: impl std::io::Read) -> Result<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader
        .take(MAX_PASSWORD_BYTES + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= MAX_PASSWORD_BYTES as usize,
        "Password input is too long"
    );
    if bytes.ends_with(b"\n") {
        bytes.pop();
        if bytes.ends_with(b"\r") {
            bytes.pop();
        }
    }
    let password =
        String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("Password input must be UTF-8"))?;
    anyhow::ensure!(
        !password.is_empty() && !password.contains(['\n', '\r', '\0']),
        "Provide exactly one non-empty password line"
    );
    Ok(password)
}

pub fn handle_login(email: &str, password_stdin: bool) -> Result<()> {
    use std::io::IsTerminal;
    let password = if password_stdin {
        anyhow::ensure!(
            !std::io::stdin().is_terminal(),
            "--password-stdin requires redirected input; omit it for the hidden prompt"
        );
        read_password(std::io::stdin().lock())?
    } else {
        rpassword::prompt_password("Tapo password: ")
            .map_err(|e| anyhow::anyhow!("Failed to read password: {e}"))?
    };
    creds::save(email, &password)?;
    crate::output::record(serde_json::json!({"credentials_saved":true}));
    crate::output::println!(
        "Tapo credentials saved to {}",
        terminal_text(&creds::path_display())
    );
    crate::output::println!(
        "(On Unix, only your account can read this file. Use TAPO_USER/TAPO_PASS env vars to override.)"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_stdin_preserves_spaces_and_removes_only_line_ending() {
        assert_eq!(
            read_password(&b"  synthetic secret  \r\n"[..]).unwrap(),
            "  synthetic secret  "
        );
        assert_eq!(read_password(&b"synthetic"[..]).unwrap(), "synthetic");
    }
    #[test]
    fn password_stdin_rejects_empty_multiline_and_oversized_input_without_echo() {
        for bytes in [
            b"".to_vec(),
            b"\n".to_vec(),
            b"sentinel\nsecret".to_vec(),
            vec![b'x'; 4097],
            vec![255],
            b"secret\0".to_vec(),
        ] {
            let error = read_password(bytes.as_slice()).unwrap_err().to_string();
            assert!(!error.contains("sentinel"));
            assert!(!error.contains("secret"));
        }
    }
}
