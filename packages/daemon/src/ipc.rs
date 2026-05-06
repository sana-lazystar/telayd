//! Unix Domain Socket IPC listener.
//!
//! The hook script (`telayd-hook-emit.sh`) writes a single newline-terminated
//! JSON line to `~/.config/telayd/daemon.sock`.  This module:
//!
//! 1. Binds + listens on that socket (chmod 0600).
//! 2. Reads line-delimited JSON frames (max 256 KiB per line).
//! 3. Parses + validates the Claude Code `PreToolUse` hook payload.
//! 4. Converts it to an internal [`Inquiry`] record.
//! 5. Sends it to the WS bridge via an `mpsc` channel.
//!
//! Security:
//! - Socket mode 0600 — only the daemon owner can write.
//! - Stale socket file from a previous run is removed on startup.
//! - Symlink at socket path is rejected.
//! - Line length capped at 256 KiB (DoS guard).

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::protocol::Inquiry;

/// Maximum byte length of a single IPC line (256 KiB).
const MAX_PAYLOAD_BYTES: u64 = 256 * 1024;

// ── Hook payload schema ────────────────────────────────────────────────────

/// Top-level hook stdin payload from Claude Code (PreToolUse).
#[derive(Debug, serde::Deserialize)]
pub struct HookPayload {
    pub session_id: String,
    #[allow(dead_code)]
    pub transcript_path: Option<String>,
    #[allow(dead_code)]
    pub cwd: Option<String>,
    pub permission_mode: Option<String>,
    pub hook_event_name: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
    pub tool_use_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ToolInput {
    pub questions: Vec<HookQuestion>,
}

#[derive(Debug, serde::Deserialize)]
pub struct HookQuestion {
    pub question: String,
    pub header: Option<String>,
    pub options: Vec<HookOption>,
    #[serde(default)]
    pub multi_select: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct HookOption {
    pub label: String,
    pub description: Option<String>,
}

// ── Validation ──────────────────────────────────────────────────────────────

/// Validates that `s` matches `^[A-Za-z0-9_-]{1,128}$`.
fn is_valid_session_id(s: &str) -> bool {
    let len = s.len();
    len >= 1
        && len <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Validates that `s` matches `^toolu_[A-Za-z0-9]{1,64}$`.
fn is_valid_tool_use_id(s: &str) -> bool {
    s.starts_with("toolu_")
        && {
            let rest = &s["toolu_".len()..];
            let len = rest.len();
            len >= 1 && len <= 64 && rest.bytes().all(|b| b.is_ascii_alphanumeric())
        }
}

impl HookPayload {
    /// Validates field values against the security whitelist rules.
    pub fn validate(&self) -> Result<()> {
        if !is_valid_session_id(&self.session_id) {
            anyhow::bail!("invalid session_id format");
        }
        if !is_valid_tool_use_id(&self.tool_use_id) {
            anyhow::bail!("invalid tool_use_id format: {}", self.tool_use_id);
        }
        if self.tool_name != "AskUserQuestion" {
            anyhow::bail!("unexpected tool_name: {}", self.tool_name);
        }
        if self.hook_event_name != "PreToolUse" {
            anyhow::bail!("unexpected hook_event_name: {}", self.hook_event_name);
        }
        if self.tool_input.questions.is_empty() {
            anyhow::bail!("questions array is empty");
        }
        if self.tool_input.questions.len() > 10 {
            anyhow::bail!("too many questions (DoS guard)");
        }
        for q in &self.tool_input.questions {
            if q.question.len() > 4096 {
                anyhow::bail!("question text too long");
            }
            if q.options.len() > 32 {
                anyhow::bail!("too many options (DoS guard)");
            }
            for opt in &q.options {
                if opt.label.len() > 256 {
                    anyhow::bail!("option label too long");
                }
                if opt.description.as_deref().unwrap_or("").len() > 1024 {
                    anyhow::bail!("option description too long");
                }
            }
        }
        Ok(())
    }

    /// Converts validated payload to an internal `Inquiry` record.
    pub fn into_inquiry(self, tmux_session: String) -> Inquiry {
        use crate::protocol::{InquiryOption, InquiryQuestion};
        let header = self
            .tool_input
            .questions
            .first()
            .and_then(|q| q.header.clone())
            .unwrap_or_default();

        let questions = self
            .tool_input
            .questions
            .into_iter()
            .map(|q| InquiryQuestion {
                question: q.question,
                options: q
                    .options
                    .into_iter()
                    .enumerate()
                    .map(|(i, o)| InquiryOption {
                        index: (i + 1) as u32,
                        label: o.label,
                        description: o.description.unwrap_or_default(),
                    })
                    .collect(),
                multi_select: q.multi_select,
            })
            .collect();

        Inquiry {
            kind: "inquiry".to_string(),
            tool_use_id: self.tool_use_id,
            session_id: self.session_id,
            tmux_session,
            header,
            questions,
            permission_mode: self.permission_mode,
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}

// ── IPC Listener ────────────────────────────────────────────────────────────

/// Starts the Unix socket IPC listener.
///
/// Sends parsed `Inquiry` records to `tx`. The caller (daemon supervisor)
/// forwards them to the WS bridge.
///
/// # Cancellation
/// The future resolves when `cancellation_token` is cancelled.
pub async fn run_ipc_listener(
    sock_path: std::path::PathBuf,
    tx: mpsc::Sender<Inquiry>,
    cancellation_token: Arc<tokio_util::sync::CancellationToken>,
    tmux_session: String,
) -> Result<()> {
    // Remove stale socket from a previous run.
    if sock_path.exists() {
        if sock_path.symlink_metadata()?.file_type().is_symlink() {
            anyhow::bail!("refusing symlink at sock path {:?}", sock_path);
        }
        std::fs::remove_file(&sock_path)
            .with_context(|| format!("remove stale sock {:?}", sock_path))?;
    }

    let listener = UnixListener::bind(&sock_path)
        .with_context(|| format!("bind unix socket {:?}", sock_path))?;

    // chmod 0600
    std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("chmod 0600 {:?}", sock_path))?;

    info!(target: "ipc", sock = ?sock_path, "IPC listener started");

    loop {
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                info!(target: "ipc", "IPC listener shutting down");
                let _ = std::fs::remove_file(&sock_path);
                return Ok(());
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        let tx2 = tx.clone();
                        let session = tmux_session.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, tx2, session).await {
                                warn!(target: "ipc", err = %e, "IPC connection error");
                            }
                        });
                    }
                    Err(e) => {
                        error!(target: "ipc", err = %e, "accept error");
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    tx: mpsc::Sender<Inquiry>,
    tmux_session: String,
) -> Result<()> {
    let mut reader = BufReader::new(stream.take(MAX_PAYLOAD_BYTES + 1));
    let mut line = Vec::with_capacity(8192);

    reader
        .read_until(b'\n', &mut line)
        .await
        .context("read IPC line")?;

    if line.len() as u64 > MAX_PAYLOAD_BYTES {
        anyhow::bail!("IPC payload exceeds 256 KiB limit");
    }

    if line.is_empty() {
        debug!(target: "ipc", "empty IPC line, ignoring");
        return Ok(());
    }

    let payload: HookPayload = serde_json::from_slice(&line).context("parse hook payload JSON")?;
    debug!(
        target: "ipc",
        tool_use_id = %payload.tool_use_id,
        session_id = %payload.session_id,
        "received hook payload"
    );

    payload.validate().context("validate hook payload")?;
    let inquiry = payload.into_inquiry(tmux_session);

    tx.send(inquiry).await.context("send inquiry to WS bridge")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build a minimal valid HookPayload from JSON
    fn valid_payload_json(tool_use_id: &str, session_id: &str) -> String {
        format!(
            r#"{{
                "session_id": "{session_id}",
                "hook_event_name": "PreToolUse",
                "tool_name": "AskUserQuestion",
                "tool_use_id": "{tool_use_id}",
                "tool_input": {{
                    "questions": [
                        {{
                            "question": "Which database?",
                            "header": "Database",
                            "options": [
                                {{"label": "PostgreSQL", "description": "RDBMS"}},
                                {{"label": "SQLite", "description": "embedded"}}
                            ],
                            "multi_select": false
                        }}
                    ]
                }}
            }}"#
        )
    }

    #[test]
    fn parse_valid_payload() {
        let json = valid_payload_json("toolu_ABC123DEF456GHI789JKL012MNO345PQR678", "abc123");
        let p: HookPayload = serde_json::from_str(&json).unwrap();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn reject_invalid_tool_use_id() {
        let json = valid_payload_json("bad-id", "abc123");
        let p: HookPayload = serde_json::from_str(&json).unwrap();
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_invalid_session_id() {
        let json = valid_payload_json("toolu_VALID12345678901234567890", "ab;cd");
        let p: HookPayload = serde_json::from_str(&json).unwrap();
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_wrong_tool_name() {
        let raw = r#"{
            "session_id": "abc123",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_use_id": "toolu_VALID1234567890123456789",
            "tool_input": {"questions": [{"question": "q?", "options": [{"label": "A"}], "multi_select": false}]}
        }"#;
        let p: HookPayload = serde_json::from_str(raw).unwrap();
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_empty_questions() {
        let raw = r#"{
            "session_id": "abc123",
            "hook_event_name": "PreToolUse",
            "tool_name": "AskUserQuestion",
            "tool_use_id": "toolu_VALID1234567890123456789",
            "tool_input": {"questions": []}
        }"#;
        let p: HookPayload = serde_json::from_str(raw).unwrap();
        assert!(p.validate().is_err());
    }

    #[test]
    fn into_inquiry_assigns_indexes_from_one() {
        let json = valid_payload_json("toolu_ABC123DEF456GHI789JKL012MNO345PQR678", "sess001");
        let p: HookPayload = serde_json::from_str(&json).unwrap();
        let inquiry = p.into_inquiry("my-session".to_string());
        assert_eq!(inquiry.questions[0].options[0].index, 1);
        assert_eq!(inquiry.questions[0].options[1].index, 2);
        assert_eq!(inquiry.tmux_session, "my-session");
    }

    #[tokio::test]
    async fn socket_listener_round_trip() {
        use tokio::io::AsyncWriteExt;
        use tokio::sync::mpsc;

        let dir = tempfile::TempDir::new().unwrap();
        let sock = dir.path().join("daemon.sock");
        let (tx, mut rx) = mpsc::channel(8);
        let token = Arc::new(tokio_util::sync::CancellationToken::new());
        let token_clone = token.clone();

        // Spawn listener
        let sock_clone = sock.clone();
        let handle = tokio::spawn(async move {
            run_ipc_listener(sock_clone, tx, token_clone, "test-session".to_string()).await
        });

        // Give it a moment to bind
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Connect and send a valid payload
        let json = valid_payload_json("toolu_TESTPAYLOAD12345678901234567", "testsess");
        let mut conn = tokio::net::UnixStream::connect(&sock).await.unwrap();
        conn.write_all(json.as_bytes()).await.unwrap();
        conn.write_all(b"\n").await.unwrap();
        conn.flush().await.unwrap();
        drop(conn);

        // Receive inquiry
        let inquiry = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("channel closed");

        assert_eq!(inquiry.tool_use_id, "toolu_TESTPAYLOAD12345678901234567");
        assert_eq!(inquiry.tmux_session, "test-session");

        // Shutdown
        token.cancel();
        let _ = handle.await;
    }
}
