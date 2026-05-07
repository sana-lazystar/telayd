//! Shared path utilities for the telayd daemon.
//!
//! All daemon-related paths resolve under `~/.config/telayd/`.
//! The config directory is created and its permissions enforced
//! (0700 for directory, 0600 for files) before first use.

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Returns `~/.config/telayd/`, creating it with mode 0700 if absent.
///
/// If the directory exists but has weaker permissions than 0700, they
/// are tightened. A wider-than-expected mode is treated as suspicious
/// but is corrected (not aborted) at this layer — callers may add
/// stricter policy if needed.
pub fn config_dir() -> Result<PathBuf> {
    let base = directories::BaseDirs::new()
        .context("could not determine home directory")?;
    let dir = base.config_dir().join("telayd");
    fs::create_dir_all(&dir).with_context(|| format!("create_dir_all {dir:?}"))?;

    // Ensure the directory mode is exactly 0700.
    let meta = fs::metadata(&dir)?;
    let mode = meta.permissions().mode() & 0o777;
    if mode != 0o700 {
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("chmod 0700 {dir:?}"))?;
    }
    Ok(dir)
}

/// Returns `~/.config/telayd/config.toml`.
pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Returns `~/.config/telayd/daemon.sock`.
pub fn daemon_sock() -> Result<PathBuf> {
    Ok(config_dir()?.join("daemon.sock"))
}

/// Returns `~/.config/telayd/daemon.pid`.
pub fn daemon_pid() -> Result<PathBuf> {
    Ok(config_dir()?.join("daemon.pid"))
}

/// Returns `~/.config/telayd/daemon.log` (used by `telayd logs`).
pub fn daemon_log() -> Result<PathBuf> {
    Ok(config_dir()?.join("daemon.log"))
}

/// Returns `~/.local/bin/` (XDG user binary directory).
pub fn local_bin() -> Result<PathBuf> {
    let base = directories::BaseDirs::new()
        .context("could not determine home directory")?;
    // `BaseDirs::executable_dir()` returns `~/.local/bin` on Linux/macOS.
    let dir = base
        .executable_dir()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            base.home_dir().join(".local").join("bin")
        });
    fs::create_dir_all(&dir).with_context(|| format!("create_dir_all {dir:?}"))?;
    Ok(dir)
}

/// Writes `content` to `path` with mode 0600 (atomic via tmp + rename).
///
/// This prevents a partial write from leaving a truncated secret file
/// on a mid-write crash.
pub fn write_secret_file(path: &std::path::Path, content: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    // Write to a sibling temp file first, then atomically rename.
    let dir = path.parent().context("path has no parent")?;
    let tmp = dir.join(format!(
        ".tmp-{}-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("telayd"),
        std::process::id()
    ));

    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .with_context(|| format!("open tmp {tmp:?}"))?;
        f.write_all(content)
            .with_context(|| format!("write {tmp:?}"))?;
    }

    fs::rename(&tmp, path)
        .with_context(|| format!("rename {tmp:?} -> {path:?}"))?;
    Ok(())
}

/// Reads and enforces the permission of an existing sensitive file.
///
/// Returns an error if the file mode is not 0600. This is intentional:
/// we treat unexpected modes as a signal of tampering and refuse to
/// start, rather than silently correcting (which would hide the event).
pub fn assert_secret_file_mode(path: &std::path::Path) -> Result<()> {
    let meta = fs::metadata(path)
        .with_context(|| format!("stat {path:?}"))?;
    if meta.file_type().is_symlink() {
        anyhow::bail!("refusing to use symlink at {path:?} (possible tamper)");
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode != 0o600 {
        anyhow::bail!(
            "unexpected permissions on {path:?}: {mode:04o} (expected 0600); \
             possible tamper — refusing to proceed"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_secret_creates_with_correct_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("secret.toml");
        write_secret_file(&path, b"token = \"test\"").unwrap();
        let meta = fs::metadata(&path).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "file should be created with mode 0600");
    }

    #[test]
    fn assert_secret_file_mode_rejects_wide_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("wide.toml");
        // Create with wide mode
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .mode(0o644)
                .open(&path)
                .unwrap();
            use std::io::Write;
            f.write_all(b"data").unwrap();
        }
        let result = assert_secret_file_mode(&path);
        assert!(result.is_err(), "should reject mode 0644");
    }
}
