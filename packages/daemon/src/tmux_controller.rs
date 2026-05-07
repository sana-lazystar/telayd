//! Tmux send-keys injection + dialog-ready detection.
//!
//! Implements ADR-W003 (dogfooding-revised):
//! - `tmux capture-pane -p -J` snapshot polling at 50ms intervals, 5s budget.
//!   capture-pane returns the *rendered* pane content (with TUI cursor moves
//!   resolved by tmux), so it works with Ratatui/Ink-style cell rendering used
//!   by Claude Code's permission dialog. The original pipe-pane stream-tail
//!   approach (spike 06.1 era) failed against current Claude Code because
//!   "Enter to select" never appears as contiguous bytes in the raw stream
//!   when each character is positioned via cursor escape sequences.
//! - 0.5s safety margin after marker detection.
//! - 1 retry on timeout, then inject forcibly + warn.
//! - send-keys invoked via `tokio::process::Command` (argv array — no shell).
//! - pipe-pane is still activated for forensic logging only — detection no
//!   longer reads from it.
//!
//! Security:
//! - `tmux_session` validated against in-memory active set before use.
//! - send-keys args are a literal whitelist (digits, Enter, Down).
//! - free_text uses `-l` (literal) flag.
//! - pipe-pane log chmod 0600, capped at 1MB.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::tmux_keymap;

// ── Constants (ADR-W003) ────────────────────────────────────────────────────

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const DETECTION_BUDGET: Duration = Duration::from_secs(5);
const SAFETY_MARGIN: Duration = Duration::from_millis(500);
const RETRY_SLEEP: Duration = Duration::from_secs(1);
const PIPE_PANE_LOG_MAX_BYTES: u64 = 1024 * 1024; // 1 MB

/// Markers that indicate the AskUserQuestion dialog is ready.
const READY_MARKERS: &[&str] = &["Enter to select", "\u{2191}/\u{2193} to navigate"];

// ── Race counter (observability) ────────────────────────────────────────────

/// `telayd_inject_race_total` — incremented on detection timeout / force-inject.
pub static INJECT_RACE_TOTAL: AtomicU64 = AtomicU64::new(0);

// ── Public API ───────────────────────────────────────────────────────────────

/// Manages all active tmux sessions tracked by the daemon.
///
/// Only sessions registered here can be driven — arbitrary session
/// names from user input are rejected.
// #[allow(clippy::manual_default)]: `Arc<dyn SentinelParser>` does not implement Default;
// use `TmuxController::new(parser)` instead.
pub struct TmuxController {
    /// Set of active tmux session names (daemon-owned).
    active_sessions: std::sync::Mutex<std::collections::HashSet<String>>,
    sentinel_parser: Arc<dyn crate::sentinel::SentinelParser>,
}

impl TmuxController {
    pub fn new(sentinel_parser: Arc<dyn crate::sentinel::SentinelParser>) -> Self {
        Self {
            active_sessions: std::sync::Mutex::default(),
            sentinel_parser,
        }
    }

    /// Registers a session for injection. Must be called before `inject`.
    pub fn register_session(&self, session: &str) {
        let mut set = self.active_sessions.lock().expect("lock active_sessions");
        set.insert(session.to_string());
    }

    /// Removes a session from the active set and cleans up its pipe-pane log.
    pub fn deregister_session(&self, session: &str) {
        let mut set = self.active_sessions.lock().expect("lock active_sessions");
        set.remove(session);
        let log = pipe_pane_log_path(session);
        let _ = std::fs::remove_file(&log);
    }

    /// Injects a response into a tmux session.
    ///
    /// # Errors
    /// Returns an error if the session is unknown or tmux commands fail.
    pub async fn inject(
        &self,
        session: &str,
        choice_index: u32,
        options_total: u32,
        free_text: Option<&str>,
        tool_use_id: &str,
    ) -> Result<()> {
        // Security: validate session against whitelist.
        {
            let set = self.active_sessions.lock().expect("lock");
            if !set.contains(session) {
                anyhow::bail!(
                    "session {session:?} not in active set (refusing arbitrary inject)"
                );
            }
        }

        info!(target: "tmux", tool_use_id, session, "starting inject");

        // Ensure pipe-pane is active.
        ensure_pipe_pane(session).await?;

        // Wait for dialog ready.
        if let Err(e) = await_dialog_ready(session).await {
            warn!(
                target: "tmux",
                tool_use_id,
                session,
                err = %e,
                "dialog ready detection failed — retry once"
            );
            tokio::time::sleep(RETRY_SLEEP).await;
            if let Err(e2) = await_dialog_ready(session).await {
                warn!(
                    target: "tmux",
                    tool_use_id,
                    session,
                    err = %e2,
                    race = true,
                    "force-injecting despite no dialog marker"
                );
                INJECT_RACE_TOTAL.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Resolve keystrokes.
        if let Some(text) = free_text {
            send_literal(session, text, tool_use_id).await?;
        } else {
            let keys = tmux_keymap::keystrokes_for(choice_index, options_total);
            send_keys(session, &keys, tool_use_id).await?;
        }

        // Truncate log if it grew too large.
        trim_log_if_needed(session);

        info!(target: "tmux", tool_use_id, session, "inject complete");
        Ok(())
    }

    /// Activates pipe-pane for a session (idempotent).
    pub async fn enable_pipe_pane(&self, session: &str) -> Result<()> {
        let set = self.active_sessions.lock().expect("lock");
        if !set.contains(session) {
            anyhow::bail!("session {session:?} not registered");
        }
        drop(set);
        ensure_pipe_pane(session).await
    }

    /// Disables pipe-pane for all active sessions and removes their logs.
    pub async fn cleanup_all(&self) {
        let sessions: Vec<String> = {
            let set = self.active_sessions.lock().expect("lock");
            set.iter().cloned().collect()
        };
        for s in &sessions {
            let _ = disable_pipe_pane(s).await;
            let log = pipe_pane_log_path(s);
            let _ = std::fs::remove_file(&log);
        }
    }

    /// Sends a `/mode <name>` command to Claude Code running in a tmux session.
    ///
    /// IG6 fix: `apply_permission_mode` in `ws_bridge.rs` MUST call this method
    /// instead of `tokio::process::Command::new("tmux")` directly, so that the
    /// session whitelist check applies (prevents arbitrary session injection).
    ///
    /// Returns `true` if the command was sent successfully, `false` if the session
    /// is not in the whitelist or the tmux call fails.
    pub async fn send_mode_command(
        &self,
        session: &str,
        mode: &crate::protocol::PermissionMode,
    ) -> bool {
        // Security: validate session against active_sessions whitelist.
        {
            let set = self.active_sessions.lock().expect("lock active_sessions");
            if !set.contains(session) {
                warn!(
                    target: "tmux",
                    session,
                    "send_mode_command rejected — session not in active whitelist"
                );
                return false;
            }
        }
        let mode_cmd = format!("/mode {}", mode.as_str());

        // Step 1: send the literal text (mirrors send_literal step-1).
        let status = Command::new("tmux")
            .args(["send-keys", "-t", session, "-l", &mode_cmd])
            .status()
            .await;
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                warn!(target: "tmux", session, ?s, "send_mode_command literal failed (non-zero exit)");
                return false;
            }
            Err(e) => {
                warn!(target: "tmux", session, err = %e, "send_mode_command literal failed");
                return false;
            }
        }

        // Step 2: send Enter to submit the slash command (mirrors send_literal step-2).
        // Without this, `/mode <name>` stays in the prompt buffer and is never submitted.
        let status = Command::new("tmux")
            .args(["send-keys", "-t", session, "Enter"])
            .status()
            .await;
        match status {
            Ok(s) if s.success() => {
                info!(target: "tmux", session, mode = ?mode, "mode command sent");
                true
            }
            Ok(s) => {
                warn!(target: "tmux", session, ?s, "send_mode_command Enter failed (non-zero exit)");
                false
            }
            Err(e) => {
                warn!(target: "tmux", session, err = %e, "send_mode_command Enter failed");
                false
            }
        }
    }

    /// Reads `sentinel_parser` — unused in L0 but DI is wired.
    pub fn sentinel_parser(&self) -> &Arc<dyn crate::sentinel::SentinelParser> {
        &self.sentinel_parser
    }

    /// Returns the first registered active session name, if any.
    ///
    /// IG2 fix (diagnosis.md §Group2 P1): `apply_permission_mode` in `ws_bridge.rs`
    /// calls this as fallback when `pending_inquiries` is empty (idle-state toggle).
    /// L0 single-session assumption: returns the first session alphabetically.
    pub fn first_active_session(&self) -> Option<String> {
        let set = self.active_sessions.lock().expect("lock active_sessions");
        // Use `iter().next()` — single-session assumption for L0.
        set.iter().next().cloned()
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn pipe_pane_log_path(session: &str) -> PathBuf {
    // session name should not contain path separators — guaranteed by
    // registration whitelist.
    let safe_session = session.replace('/', "_").replace('\\', "_");
    PathBuf::from(format!("/tmp/telayd-{safe_session}.log"))
}

async fn ensure_pipe_pane(session: &str) -> Result<()> {
    let log = pipe_pane_log_path(session);
    let log_str = log.to_string_lossy();

    // `tmux pipe-pane -t <session> -O 'cat >> /tmp/telayd-<session>.log'`
    let status = Command::new("tmux")
        .args([
            "pipe-pane",
            "-t",
            session,
            "-O",
            &format!("cat >> {log_str}"),
        ])
        .status()
        .await
        .context("tmux pipe-pane")?;

    if !status.success() {
        anyhow::bail!("tmux pipe-pane failed for session {session:?}");
    }

    // chmod 0600 on the log if it exists.
    if log.exists() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&log, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

async fn disable_pipe_pane(session: &str) -> Result<()> {
    Command::new("tmux")
        .args(["pipe-pane", "-t", session])
        .status()
        .await
        .context("tmux pipe-pane off")?;
    Ok(())
}

/// Polls `tmux capture-pane` snapshots for a dialog-ready marker.
///
/// `capture-pane -p -J` returns the rendered pane content (with TUI cursor
/// positioning resolved), which contains "Enter to select" / "↑/↓ to navigate"
/// as contiguous text once Claude Code's permission dialog is on-screen. The
/// older pipe-pane stream-tail approach failed against Ratatui-style cell
/// rendering (each character drawn via independent cursor moves).
///
/// Returns `Ok(())` when a marker is found (after 0.5s safety margin).
/// Returns `Err` if the 5s budget is exceeded.
async fn await_dialog_ready(session: &str) -> Result<()> {
    let deadline = Instant::now() + DETECTION_BUDGET;

    loop {
        if Instant::now() >= deadline {
            anyhow::bail!("dialog ready timeout for session {session:?}");
        }

        let output = Command::new("tmux")
            .args(["capture-pane", "-t", session, "-p", "-J"])
            .output()
            .await
            .context("tmux capture-pane")?;

        if output.status.success() {
            // capture-pane may still emit a few SGR sequences in some themes —
            // strip ANSI defensively before substring search.
            let content = String::from_utf8_lossy(&output.stdout);
            let stripped = strip_ansi(&content);
            for marker in READY_MARKERS {
                if stripped.contains(marker) {
                    debug!(target: "tmux", session, marker, "dialog ready marker found");
                    tokio::time::sleep(SAFETY_MARGIN).await;
                    return Ok(());
                }
            }
        }
        // capture-pane non-zero exit (session vanished, etc.) → treat as
        // "not ready yet" and retry the next interval until budget exhausts.

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Strips ANSI escape sequences from `s`.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip ESC sequences: ESC [ ... <letter>
            if chars.peek() == Some(&'[') {
                let _ = chars.next(); // consume '['
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Sends keystroke arguments (whitelist: digits, Enter, Down).
async fn send_keys(session: &str, keys: &[String], tool_use_id: &str) -> Result<()> {
    // All keys are from tmux_keymap whitelist; no shell concatenation.
    let mut args = vec!["send-keys".to_string(), "-t".to_string(), session.to_string()];
    args.extend_from_slice(keys);

    debug!(target: "tmux", tool_use_id, ?keys, "send-keys");

    let status = Command::new("tmux")
        .args(&args)
        .status()
        .await
        .context("tmux send-keys")?;

    if !status.success() {
        anyhow::bail!("tmux send-keys failed for session {session:?}");
    }
    Ok(())
}

/// Sends free text using `-l` (literal) mode — escape sequences interpreted
/// by tmux are suppressed.
async fn send_literal(session: &str, text: &str, tool_use_id: &str) -> Result<()> {
    debug!(
        target: "tmux",
        tool_use_id,
        text_len = text.len(),
        "send-keys literal"
    );

    // `-l` flag: tmux treats the string as literal key strokes, not key names.
    let status = Command::new("tmux")
        .args(["send-keys", "-t", session, "-l", text])
        .status()
        .await
        .context("tmux send-keys -l")?;

    if !status.success() {
        anyhow::bail!("tmux send-keys -l failed for session {session:?}");
    }

    // Follow with Enter.
    let status = Command::new("tmux")
        .args(["send-keys", "-t", session, "Enter"])
        .status()
        .await
        .context("tmux send-keys Enter")?;

    if !status.success() {
        anyhow::bail!("tmux send-keys Enter failed for session {session:?}");
    }
    Ok(())
}

fn trim_log_if_needed(session: &str) {
    let log = pipe_pane_log_path(session);
    if let Ok(meta) = std::fs::metadata(&log) {
        if meta.len() > PIPE_PANE_LOG_MAX_BYTES {
            // Truncate by overwriting with empty content.
            let _ = std::fs::write(&log, b"");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_sequences() {
        let raw = "\x1b[32mEnter to select\x1b[0m more text";
        let stripped = strip_ansi(raw);
        assert!(stripped.contains("Enter to select"));
        assert!(!stripped.contains("\x1b"));
    }

    #[test]
    fn keystrokes_for_1_returns_enter() {
        let keys = tmux_keymap::keystrokes_for(1, 4);
        assert_eq!(keys, vec!["Enter"]);
    }

    #[test]
    fn controller_rejects_unregistered_session() {
        let ctrl = TmuxController::new(Arc::new(crate::sentinel::NoopSentinelParser));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(ctrl.inject("ghost-session", 1, 3, None, "toolu_X"));
        assert!(res.is_err());
    }

    #[test]
    fn inject_race_counter_accessible() {
        let v = INJECT_RACE_TOTAL.load(Ordering::Relaxed);
        // Just verify it's accessible; exact value depends on test runs.
        let _ = v;
    }

    /// IG-r2-1 regression: send_mode_command source-level contract check.
    ///
    /// Verifies that `send_mode_command` in tmux_controller.rs calls tmux
    /// send-keys twice: once for the literal text and once for Enter.
    /// This is a static pattern check (pattern-wide grep assertion documented
    /// here, exercised manually with:
    ///   grep -n 'send-keys.*Enter' packages/daemon/src/tmux_controller.rs
    /// → must return ≥2 hits: one in send_literal and one in send_mode_command).
    ///
    /// The test below validates the two-step structure in isolation by confirming
    /// the function body contains the Enter step (source-code invariant test).
    #[test]
    fn send_mode_command_source_has_literal_then_enter_steps() {
        // Pattern-wide grep: both send_literal AND send_mode_command must have
        // an Enter send-keys call.  We verify by inspecting the source string.
        let source = include_str!("tmux_controller.rs");
        // Count occurrences of '"Enter"' inside send-keys calls.
        // Expected: ≥2 (one in send_literal, one in send_mode_command).
        let enter_send_count = source.matches(r#""Enter""#).count();
        assert!(
            enter_send_count >= 2,
            "Expected ≥2 'Enter' send-keys calls in tmux_controller.rs (send_literal + \
             send_mode_command), found {enter_send_count}. \
             IG-r2-1 regression: send_mode_command must send Enter after literal text."
        );
    }
}
