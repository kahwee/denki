//! Atomic replacement and process-shared advisory locks for local configuration.
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Keep this handle alive for the entire read-modify-write transaction.
/// The stable sidecar is never removed: replacing/unlinking it would split locks.
pub(crate) struct FileLock {
    _file: File,
}
impl FileLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self> {
        let parent = path.parent().context("Configuration path has no parent")?;
        std::fs::create_dir_all(parent)?;
        let mut name = path.as_os_str().to_os_string();
        name.push(".lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(Path::new(&name))
            .context("Could not open configuration lock")?;
        file.lock().context("Could not lock configuration")?;
        Ok(Self { _file: file })
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_with(path, |file| {
        file.write_all(bytes)?;
        Ok(())
    })
}

fn atomic_write_with(path: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let parent = path.parent().context("Configuration path has no parent")?;
    std::fs::create_dir_all(parent)?;
    // tempfile creates with mode 0600 on Unix, before any secret is written.
    // Same-directory placement guarantees rename stays on the same filesystem.
    let mut temporary = tempfile::Builder::new()
        .prefix(".denki-")
        .tempfile_in(parent)?;
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .context("Could not replace configuration atomically")?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_write_preserves_original_and_cleans_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"original").unwrap();
        let result = atomic_write_with(&path, |file| {
            file.write_all(b"partial")?;
            anyhow::bail!("simulated write failure")
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn failed_replace_cleans_temporary_without_removing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("occupied");
        std::fs::create_dir(&path).unwrap();
        assert!(atomic_write(&path, b"new").is_err());
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn temporary_is_private_before_first_write_and_replaces_permissive_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        atomic_write_with(&path, |file| {
            assert_eq!(file.metadata()?.permissions().mode() & 0o777, 0o600);
            file.write_all(b"test-secret")?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"test-secret");
    }
    #[cfg(unix)]
    #[test]
    fn replacement_does_not_follow_destination_symlink() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("unrelated");
        let path = dir.path().join("credentials.json");
        std::fs::write(&target, b"unchanged").unwrap();
        symlink(&target, &path).unwrap();
        atomic_write(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");
        assert!(!std::fs::symlink_metadata(&path).unwrap().is_symlink());
    }
}
