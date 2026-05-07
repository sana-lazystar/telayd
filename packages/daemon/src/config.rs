//! `~/.config/telayd/config.toml` — persistent daemon configuration.
//!
//! Schema:
//! ```toml
//! pairing_token = "<43-char base64url no-padding>"
//! last_tunnel_url = "https://random.trycloudflare.com"
//! created_at = "2026-05-06T12:34:56Z"
//! ```
//!
//! Security:
//! - Written with mode 0600 via atomic rename (tmpfile → rename).
//! - On load, file mode is asserted to be exactly 0600.
//! - Symlinks at the config path are rejected.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

/// The daemon's persistent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 43-char base64url no-padding pairing token.
    pub pairing_token: String,
    /// Last known cloudflared quick tunnel URL (may be stale after rotation).
    #[serde(default)]
    pub last_tunnel_url: Option<String>,
    /// ISO-8601 timestamp when the token was generated.
    pub created_at: String,
}

impl Config {
    /// Loads config from `~/.config/telayd/config.toml`.
    ///
    /// Fails if the file doesn't exist, has wrong permissions, or parses
    /// with an invalid token format.
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;
        Self::load_from(&path)
    }

    /// Loads config from an explicit path (useful for tests).
    ///
    /// IG7 fix (P2 — diagnosis.md §Group7): uses single-syscall pattern:
    ///   open(O_NOFOLLOW) → fstat → assert mode → read_to_string
    /// This closes the TOCTOU window between `assert_secret_file_mode`
    /// (which calls fs::metadata) and the subsequent `read_to_string` call.
    /// CWE-367 mitigation: the mode check and read happen on the same fd.
    pub fn load_from(path: &Path) -> Result<Self> {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;

        // Open with O_NOFOLLOW: fails if `path` is a symlink (ELOOP on macOS/Linux).
        // This replaces the separate symlink_metadata() + read_to_string() split.
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            // O_NOFOLLOW: reject symlink at the final component.
            // POSIX value 0x20000 (macOS) / 0x20000 (Linux) — nix constant safe.
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .with_context(|| format!("open (O_NOFOLLOW) {path:?} — may be a symlink"))?;

        // fstat on the open fd — same fd as the subsequent read (no TOCTOU window).
        let metadata = file.metadata()
            .with_context(|| format!("fstat {path:?}"))?;

        // Assert mode 0600 on the already-open fd.
        {
            use std::os::unix::fs::MetadataExt;
            let mode = metadata.mode() & 0o777;
            if mode != 0o600 {
                anyhow::bail!(
                    "unexpected permissions on {path:?}: {mode:04o} (expected 0600); \
                     possible tamper — refusing to proceed"
                );
            }
        }

        let mut content = String::new();
        file.read_to_string(&mut content)
            .with_context(|| format!("read {path:?}"))?;

        let cfg: Config = toml::from_str(&content)
            .with_context(|| format!("parse TOML {path:?}"))?;

        // Validate token format.
        if !crate::pairing::is_valid_token_format(&cfg.pairing_token) {
            anyhow::bail!("config.toml: pairing_token is not a valid 43-char base64url string");
        }
        Ok(cfg)
    }

    /// Saves config to `~/.config/telayd/config.toml` with mode 0600.
    ///
    /// Uses atomic rename to prevent partial writes.
    pub fn save(&self) -> Result<()> {
        let path = paths::config_file()?;
        self.save_to(&path)
    }

    /// Saves to an explicit path (useful for tests).
    pub fn save_to(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("serialize config to TOML")?;
        paths::write_secret_file(path, content.as_bytes())
            .with_context(|| format!("write config {path:?}"))?;
        Ok(())
    }

    /// Updates `last_tunnel_url` and persists.
    pub fn set_tunnel_url(&mut self, url: Option<String>) -> Result<()> {
        self.last_tunnel_url = url;
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config() -> Config {
        Config {
            pairing_token: crate::pairing::generate_token().unwrap(),
            last_tunnel_url: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = test_config();
        let original_token = cfg.pairing_token.clone();

        cfg.save_to(&path).unwrap();

        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded.pairing_token, original_token);
        assert!(loaded.last_tunnel_url.is_none());
    }

    #[test]
    fn save_creates_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        test_config().save_to(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn load_fails_on_wrong_mode() {
        use std::os::unix::fs::OpenOptionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");

        // Write with mode 0644 (too permissive)
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .mode(0o644)
                .open(&path)
                .unwrap();
            let cfg = test_config();
            let content = toml::to_string_pretty(&cfg).unwrap();
            f.write_all(content.as_bytes()).unwrap();
        }
        assert!(Config::load_from(&path).is_err());
    }

    #[test]
    fn set_tunnel_url_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = test_config();
        cfg.save_to(&path).unwrap();

        // Reload and update
        let mut loaded = Config::load_from(&path).unwrap();
        let url = "https://example.trycloudflare.com".to_string();
        loaded.last_tunnel_url = Some(url.clone());
        loaded.save_to(&path).unwrap();

        let reloaded = Config::load_from(&path).unwrap();
        assert_eq!(reloaded.last_tunnel_url, Some(url));
    }
}
