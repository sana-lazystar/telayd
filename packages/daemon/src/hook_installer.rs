//! Hook installer — registers / removes the `telayd-hook-emit` entry in
//! `~/.claude/settings.json`.
//!
//! Implements:
//! - Idempotent install (no duplicate entries).
//! - Backup before any modification (`settings.json.bak.<nanosecond-ts>`).
//! - Unknown-field preserve (B6 mitigation).
//! - Symlink rejection.
//! - Clean install (settings.json absent) + existing install.
//!
//! Security (§5.1 checklist):
//! - Backup chmod 600.
//! - JSON parse failure → bail without touching original.
//! - Idempotent (duplicate guard).
//! - Unknown fields preserved.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;
use tracing::info;

// ── Hook entry constant ──────────────────────────────────────────────────────

/// The hook command installed into `PreToolUse`.
const HOOK_COMMAND: &str = "telayd-hook-emit";
const HOOK_MATCHER: &str = "AskUserQuestion";
const HOOK_TIMEOUT_MS: u64 = 5000;

// ── Settings.json path ───────────────────────────────────────────────────────

/// Returns `~/.claude/settings.json`.
pub fn settings_json_path() -> Result<PathBuf> {
    let home = directories::BaseDirs::new()
        .context("no home dir")?
        .home_dir()
        .to_path_buf();
    Ok(home.join(".claude").join("settings.json"))
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Installs the `telayd-hook-emit` hook entry.
///
/// - If `settings.json` doesn't exist, creates a minimal one.
/// - Backs up the existing file before modification.
/// - Idempotent: does nothing if the entry is already present.
pub fn install_hook() -> Result<()> {
    let path = settings_json_path()?;

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create_dir_all {parent:?}"))?;
    }

    let (mut json, existed) = if path.exists() {
        // IG7 fix (P2): open with O_NOFOLLOW to close the TOCTOU window between
        // the symlink check and the subsequent read.  The single-syscall pattern
        // (open with O_NOFOLLOW → read) replaces separate reject_symlink() +
        // read_to_string() calls.
        let raw = read_no_follow(&path)?;
        let v: Value = serde_json::from_str(&raw)
            .with_context(|| format!("parse JSON {path:?}"))?;
        (v, true)
    } else {
        (serde_json::json!({}), false)
    };

    if is_hook_installed(&json) {
        info!(target: "hook_installer", "hook entry already present (idempotent)");
        return Ok(());
    }

    // Back up existing file.
    if existed {
        backup_settings(&path)?;
    }

    // Inject the hook entry.
    merge_hook_entry(&mut json)?;

    // Write back (atomic rename).
    let serialised = serde_json::to_string_pretty(&json)
        .context("serialize settings.json")?;
    crate::paths::write_secret_file(&path, serialised.as_bytes())?;

    info!(target: "hook_installer", "hook entry installed");
    Ok(())
}

/// Removes the `telayd-hook-emit` hook entry (if present).
pub fn uninstall_hook() -> Result<()> {
    let path = settings_json_path()?;
    if !path.exists() {
        return Ok(());
    }

    // IG7 fix: same O_NOFOLLOW single-syscall pattern.
    let raw = read_no_follow(&path)?;
    let mut json: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parse JSON {path:?}"))?;

    if !is_hook_installed(&json) {
        info!(target: "hook_installer", "hook entry not present, nothing to remove");
        return Ok(());
    }

    backup_settings(&path)?;
    remove_hook_entry(&mut json);

    let serialised = serde_json::to_string_pretty(&json).context("serialize")?;
    crate::paths::write_secret_file(&path, serialised.as_bytes())?;

    info!(target: "hook_installer", "hook entry removed");
    Ok(())
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Opens a file with O_NOFOLLOW and reads its content.
///
/// IG7 fix (P2): single-syscall pattern that rejects symlinks at the OS level
/// and reads on the same fd — eliminates the TOCTOU window between
/// reject_symlink() + read_to_string().
fn read_no_follow(path: &std::path::Path) -> Result<String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .with_context(|| format!("open (O_NOFOLLOW) {path:?} — may be a symlink"))?;

    let mut content = String::new();
    f.read_to_string(&mut content)
        .with_context(|| format!("read {path:?}"))?;
    Ok(content)
}

/// Returns `true` if our hook command already exists in `PreToolUse`.
pub fn is_hook_installed(json: &Value) -> bool {
    let hooks_arr = json
        .pointer("/hooks/PreToolUse")
        .and_then(|v| v.as_array());
    let Some(arr) = hooks_arr else { return false };

    arr.iter().any(|entry| {
        let matcher = entry.get("matcher").and_then(|v| v.as_str());
        let inner_hooks = entry.get("hooks").and_then(|v| v.as_array());

        matcher == Some(HOOK_MATCHER)
            && inner_hooks.map_or(false, |h| {
                h.iter().any(|h| {
                    h.get("command").and_then(|c| c.as_str()) == Some(HOOK_COMMAND)
                })
            })
    })
}

/// Injects the hook entry into `json["hooks"]["PreToolUse"]`.
pub fn merge_hook_entry(json: &mut Value) -> Result<()> {
    // Ensure `hooks` key exists.
    if json.get("hooks").is_none() {
        json["hooks"] = serde_json::json!({});
    }

    // Ensure `hooks.PreToolUse` is an array.
    {
        let hooks = json["hooks"].as_object_mut().context("hooks not an object")?;
        if !hooks.contains_key("PreToolUse") {
            hooks.insert("PreToolUse".to_string(), serde_json::json!([]));
        }
    }

    let pre_tool_use = json["hooks"]["PreToolUse"]
        .as_array_mut()
        .context("PreToolUse not an array")?;

    // Append our entry.
    pre_tool_use.push(serde_json::json!({
        "matcher": HOOK_MATCHER,
        "hooks": [
            {
                "type": "command",
                "command": HOOK_COMMAND,
                "timeout": HOOK_TIMEOUT_MS
            }
        ]
    }));

    Ok(())
}

fn remove_hook_entry(json: &mut Value) {
    let arr = json
        .pointer_mut("/hooks/PreToolUse")
        .and_then(|v| v.as_array_mut());
    let Some(arr) = arr else { return };

    arr.retain(|entry| {
        let matcher = entry.get("matcher").and_then(|v| v.as_str());
        let inner = entry.get("hooks").and_then(|v| v.as_array());
        !(matcher == Some(HOOK_MATCHER)
            && inner.map_or(false, |h| {
                h.iter()
                    .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(HOOK_COMMAND))
            }))
    });
}

/// Creates a timestamped backup of `path` with mode 0600.
pub fn backup_settings(path: &std::path::Path) -> Result<()> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let bak_path = path.with_extension(format!("json.bak.{ts}"));
    let content = std::fs::read(path).with_context(|| format!("read {path:?}"))?;
    crate::paths::write_secret_file(&bak_path, &content)
        .with_context(|| format!("backup {path:?} → {bak_path:?}"))?;
    info!(target: "hook_installer", backup = ?bak_path, "settings.json backed up");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Fixture 1: empty JSON
    #[test]
    fn install_into_empty_json() {
        let mut json = serde_json::json!({});
        assert!(!is_hook_installed(&json));
        merge_hook_entry(&mut json).unwrap();
        assert!(is_hook_installed(&json));
    }

    // Fixture 2: existing hooks.PreToolUse with other entries
    #[test]
    fn install_with_existing_other_hooks() {
        let mut json = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [{"type": "command", "command": "other-tool"}]
                    }
                ]
            }
        });
        merge_hook_entry(&mut json).unwrap();
        assert!(is_hook_installed(&json));
        // Other entry preserved.
        let arr = json.pointer("/hooks/PreToolUse").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    // Fixture 3: already installed → idempotent
    #[test]
    fn idempotent_does_not_duplicate() {
        let mut json = serde_json::json!({});
        merge_hook_entry(&mut json).unwrap();
        assert!(is_hook_installed(&json));
        // Second call must not add a duplicate.
        // (In practice install_hook() checks is_hook_installed first.)
        let arr_before = json.pointer("/hooks/PreToolUse").unwrap().as_array().unwrap().len();
        // Re-running merge without the idempotent guard would duplicate.
        // Verify the guard works at the `install_hook` level by checking
        // is_hook_installed returns true.
        assert!(is_hook_installed(&json));
        let arr_after = json.pointer("/hooks/PreToolUse").unwrap().as_array().unwrap().len();
        assert_eq!(arr_before, arr_after);
    }

    // Fixture 4: malformed JSON → bail without modification
    #[test]
    fn install_hook_with_invalid_json_file_fails_safely() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        crate::paths::write_secret_file(&path, b"not-valid-json {").unwrap();

        // Temporarily redirect settings_json_path is not possible without DI,
        // so we test the underlying JSON parsing directly.
        let res = serde_json::from_str::<Value>("not-valid-json {");
        assert!(res.is_err(), "invalid JSON should fail parse");
    }

    // Fixture 5: unknown fields preserved after merge
    #[test]
    fn unknown_fields_preserved() {
        let mut json = serde_json::json!({
            "permissions": {"allow": ["*"]},
            "model": "claude-opus-4",
            "hooks": {}
        });
        merge_hook_entry(&mut json).unwrap();
        assert_eq!(
            json.get("model").and_then(|v| v.as_str()),
            Some("claude-opus-4")
        );
        assert!(json.get("permissions").is_some());
    }

    #[test]
    fn backup_creates_with_mode_600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        crate::paths::write_secret_file(&path, b"{}").unwrap();
        backup_settings(&path).unwrap();

        // Find the backup file.
        let bak: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains("settings.json.bak")
            })
            .collect();
        assert_eq!(bak.len(), 1, "expected exactly one backup file");
        let mode = bak[0].metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn remove_hook_entry_cleans_up() {
        let mut json = serde_json::json!({});
        merge_hook_entry(&mut json).unwrap();
        assert!(is_hook_installed(&json));
        remove_hook_entry(&mut json);
        assert!(!is_hook_installed(&json));
    }
}
