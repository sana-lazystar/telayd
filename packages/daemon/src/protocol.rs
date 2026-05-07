//! WebSocket message envelope schema — L0 lock (9 message types).
//!
//! SSOT: architecture.md §2.3 / api-spec.md.
//! Self-contained copy for L0 (no shared crate with PWA).
//!
//! # Envelope
//! All messages share a common envelope:
//! ```json
//! { "v": 1, "type": "...", "id": "...", "ts": "...", "payload": {...} }
//! ```

use serde::{Deserialize, Serialize};

// ── Envelope ────────────────────────────────────────────────────────────────

/// Common outer envelope for every WS frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Protocol version. L0 = 1.
    pub v: u32,
    /// Message type (kebab-case, 9-type whitelist).
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Correlation ID: UUID v4 or `toolu_...` for inquiries.
    pub id: String,
    /// ISO-8601 timestamp with milliseconds.
    pub ts: String,
    /// Type-specific payload.
    pub payload: serde_json::Value,
}

impl Envelope {
    pub fn new(msg_type: impl Into<String>, id: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            v: 1,
            msg_type: msg_type.into(),
            id: id.into(),
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            payload,
        }
    }
}

/// The 9 allowed message type strings.
pub const ALLOWED_TYPES: &[&str] = &[
    "pairing-request",
    "pairing-ack",
    "pairing-reject",
    "inquiry-push",
    "inquiry-response",
    "inquiry-ack",
    "inquiry-error",
    "mode-toggle-request",
    "mode-toggle-ack",
];

// ── Pairing ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PairingRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct PairingAck {
    pub session: String,
    pub server_version: String,
}

/// Typed reason for a pairing rejection (IG1: replaces `String` reason field).
/// Serialises as kebab-case on the wire per architecture.md §2.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PairingRejectReason {
    TokenMismatch,
    Expired,
    BadEnvelope,
    BadPayload,
    UnsupportedVersion,
}

#[derive(Debug, Serialize)]
pub struct PairingReject {
    pub reason: PairingRejectReason,
}

// ── Inquiry ──────────────────────────────────────────────────────────────────

/// Internal Inquiry record (telayd-protocol §3.2).
/// Used both as the IPC-received object and the `inquiry-push` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inquiry {
    pub kind: String,
    pub tool_use_id: String,
    pub session_id: String,
    pub tmux_session: String,
    pub header: String,
    pub questions: Vec<InquiryQuestion>,
    /// Reflects `permission_mode` from hook payload (informational).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InquiryQuestion {
    pub question: String,
    pub options: Vec<InquiryOption>,
    /// Serialised as `multiSelect` on the wire (architecture.md §3.3 + telayd-protocol §3.2).
    /// Do NOT rename ipc::HookQuestion::multi_select — that consumes the Claude Code hook payload
    /// which uses `multiSelect` key as well (IG1 fix: diagnosis.md §Group1).
    #[serde(rename = "multiSelect")]
    pub multi_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InquiryOption {
    pub index: u32,
    pub label: String,
    pub description: String,
}

// ── Inquiry Response ─────────────────────────────────────────────────────────

/// PWA → daemon: user's answer to an inquiry.
///
/// Exactly one of `choice_index`, `free_text`, `cancel` must be set.
#[derive(Debug, Deserialize)]
pub struct InquiryResponse {
    pub tool_use_id: String,
    pub choice_index: Option<u32>,
    pub free_text: Option<String>,
    pub cancel: Option<bool>,
}

impl InquiryResponse {
    /// Ensures exactly one variant is set and values are in range.
    pub fn validate(&self, max_options: u32) -> anyhow::Result<()> {
        let n = [
            self.choice_index.is_some(),
            self.free_text.is_some(),
            self.cancel.filter(|&b| b).is_some(),
        ]
        .iter()
        .filter(|&&b| b)
        .count();

        if n != 1 {
            anyhow::bail!("inquiry-response: exactly one of choice_index/free_text/cancel must be set");
        }

        if let Some(idx) = self.choice_index {
            if idx < 1 || idx > max_options {
                anyhow::bail!("choice_index {idx} out of range 1..{max_options}");
            }
        }
        if let Some(text) = &self.free_text {
            if text.len() > 4096 {
                anyhow::bail!("free_text exceeds 4096 bytes");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct InquiryAck {
    pub tool_use_id: String,
    pub latency_ms: u64,
}

/// Typed reason for an inquiry error (IG1: replaces magic-string `reason`).
/// Serialises as kebab-case on the wire per architecture.md §2.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InquiryErrorReason {
    DialogNotReady,
    SendKeysFailed,
    InquiryStale,
    Validation,
}

#[derive(Debug, Serialize)]
pub struct InquiryError {
    pub tool_use_id: String,
    pub reason: InquiryErrorReason,
}

// ── Mode Toggle ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    Plan,
    AcceptEdits,
    Default,
}

impl PermissionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::Plan => "plan",
            PermissionMode::AcceptEdits => "accept-edits",
            PermissionMode::Default => "default",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ModeToggleRequest {
    pub mode: PermissionMode,
}

#[derive(Debug, Serialize)]
pub struct ModeToggleAck {
    pub mode: PermissionMode,
    pub applied: bool,
    /// IG2 fix: optional reason when `applied=false` (e.g. "no-session").
    /// Serialised as `null` when absent so the PWA can always check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Builds a serialized envelope text frame ready to send.
pub fn build_frame<P: Serialize>(
    msg_type: &str,
    id: &str,
    payload: &P,
) -> anyhow::Result<String> {
    let env = Envelope::new(msg_type, id, serde_json::to_value(payload)?);
    Ok(serde_json::to_string(&env)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trip_all_types() {
        for t in ALLOWED_TYPES {
            let env = Envelope::new(*t, "test-id", serde_json::json!({"key": "value"}));
            let json = serde_json::to_string(&env).unwrap();
            let parsed: Envelope = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.v, 1);
            assert_eq!(parsed.msg_type, *t);
        }
    }

    #[test]
    fn inquiry_response_validation_choice() {
        let r = InquiryResponse {
            tool_use_id: "toolu_X".to_string(),
            choice_index: Some(2),
            free_text: None,
            cancel: None,
        };
        assert!(r.validate(3).is_ok());
        assert!(r.validate(1).is_err()); // out of range
    }

    #[test]
    fn inquiry_response_validation_rejects_dual_set() {
        let r = InquiryResponse {
            tool_use_id: "toolu_X".to_string(),
            choice_index: Some(1),
            free_text: Some("text".to_string()),
            cancel: None,
        };
        assert!(r.validate(3).is_err());
    }

    #[test]
    fn inquiry_response_cancel_ok() {
        let r = InquiryResponse {
            tool_use_id: "toolu_X".to_string(),
            choice_index: None,
            free_text: None,
            cancel: Some(true),
        };
        assert!(r.validate(3).is_ok());
    }

    #[test]
    fn mode_toggle_deserialize() {
        let json = r#"{"mode": "plan"}"#;
        let req: ModeToggleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, PermissionMode::Plan);
    }
}
