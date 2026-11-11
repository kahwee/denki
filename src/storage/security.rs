//! Configuration trust boundary. Other local accounts must not be able to replace
//! inputs or lock sidecars. Same-UID processes and root remain trusted.
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;

fn unsafe_config() -> anyhow::Error {
    crate::error::error(
        "unsafe_configuration",
        "Unsafe configuration ownership, permissions, file type, or symbolic link. Keep Denki's directory private (0700), credentials private (0600), and remove group/other write access from its registry and parent directories.",
    )
}

#[cfg(unix)]
fn directory(path: &Path, create: bool, depth: usize) -> Result<bool> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    if depth > 128 {
        return Err(unsafe_config());
    }
    if let Some(parent) = path.parent()
        && parent != path
        && !directory(parent, create, depth + 1)?
    {
        return Ok(false);
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !create {
                return Ok(false);
            }
            match std::fs::DirBuilder::new().mode(0o700).create(path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error.into()),
            }
            std::fs::symlink_metadata(path)?
        }
        Err(error) => return Err(error.into()),
    };
    // Permit root-managed system redirects such as macOS /var -> /private/var.
    // The containing directory has already been checked before following them.
    if metadata.is_symlink() {
        if metadata.uid() != 0 {
            return Err(unsafe_config());
        }
        return directory(&std::fs::canonicalize(path)?, create, depth + 1);
    }
    let uid = rustix::process::geteuid().as_raw();
    let trusted_owner = metadata.uid() == 0 || metadata.uid() == uid;
    // /tmp may be shared: its sticky bit prevents another user replacing our
    // owned child directory. The immediate configuration directory cannot be shared.
    let sticky_system = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
    if !metadata.is_dir() || !trusted_owner || (metadata.mode() & 0o022 != 0 && !sticky_system) {
        return Err(unsafe_config());
    }
    Ok(true)
}

pub(super) fn parent(path: &Path, create: bool) -> Result<bool> {
    let parent = path.parent().context("Configuration path has no parent")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let absolute = std::path::absolute(parent)?;
        if !directory(&absolute, create, 0)? {
            return Ok(false);
        }
        let metadata = std::fs::symlink_metadata(&absolute)?;
        if metadata.is_symlink()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o022 != 0
        {
            return Err(unsafe_config());
        }
    }
    #[cfg(not(unix))]
    {
        if create {
            std::fs::create_dir_all(parent)?;
        }
        if !parent.exists() {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn options(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
        );
    }
    #[cfg(not(unix))]
    let _ = options;
}

pub(super) fn validate_file(file: &File, private: bool) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(unsafe_config());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let forbidden = if private { 0o077 } else { 0o022 };
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & forbidden != 0
            || metadata.nlink() != 1
        {
            return Err(unsafe_config());
        }
    }
    #[cfg(not(unix))]
    let _ = private;
    Ok(())
}

/// Check the opened object, then read a bounded amount from that same descriptor.
pub(crate) fn read(path: &Path, private: bool, limit: u64) -> Result<Option<String>> {
    if !parent(path, false)? {
        return Ok(None);
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    options(&mut opts);
    let file = match opts.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    validate_file(&file, private)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(crate::error::error(
            "configuration_too_large",
            "Configuration file exceeds its size limit",
        ));
    }
    // Never expose credential contents through decoding errors.
    Ok(Some(String::from_utf8(bytes).map_err(|_| {
        crate::error::error("malformed_configuration", "Configuration must be UTF-8")
    })?))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn unsafe_ancestors_block_reads_and_writes_even_if_child_is_private() {
        let root = tempfile::tempdir().unwrap();
        let parent_dir = root.path().join("shared");
        let private = parent_dir.join("private");
        std::fs::create_dir_all(&private).unwrap();
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = private.join("hosts.json");
        std::fs::write(&path, b"{}").unwrap();
        std::fs::set_permissions(&parent_dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(
            crate::error::code(&read(&path, false, 20).unwrap_err()),
            "unsafe_configuration"
        );
        assert!(super::super::atomic_write(&path, b"changed").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{}");
    }

    #[test]
    fn files_are_bounded_and_credentials_must_be_private() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("credentials.json");
        std::fs::write(&path, b"synthetic-secret").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read(&path, true, 100).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let error = read(&path, true, 4).unwrap_err();
        assert_eq!(crate::error::code(&error), "configuration_too_large");
        assert!(!error.to_string().contains("synthetic-secret"));
        assert_eq!(read(&path, true, 100).unwrap().unwrap(), "synthetic-secret");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert!(read(&path, false, 100).is_err());
    }

    #[test]
    fn links_and_special_files_are_rejected_without_blocking_or_modifying_targets() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        std::fs::write(&target, b"unchanged").unwrap();
        let link = root.path().join("symlink");
        symlink(&target, &link).unwrap();
        assert!(read(&link, false, 100).is_err());
        let hard = root.path().join("hardlink");
        std::fs::hard_link(&target, &hard).unwrap();
        assert!(read(&hard, false, 100).is_err());
        let fifo = root.path().join("fifo");
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        assert!(read(&fifo, false, 100).is_err());
        let config = root.path().join("config");
        symlink(&target, config.with_extension("lock")).unwrap();
        assert!(super::super::FileLock::acquire(&config).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");
    }

    #[test]
    fn new_directories_are_private_and_missing_files_remain_optional() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("new/nested/hosts.json");
        assert!(read(&path, false, 100).unwrap().is_none());
        super::super::atomic_write(&path, b"{}").unwrap();
        assert_eq!(
            path.parent()
                .unwrap()
                .metadata()
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(read(&path, false, 100).unwrap().unwrap(), "{}");
    }
}
