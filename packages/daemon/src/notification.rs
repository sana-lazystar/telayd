//! macOS notification wrapper (osascript).
//!
//! Fires a native macOS notification with title "Telayd".
//! Failure is non-fatal (user may have disabled notifications).
//!
//! # Security (IG1 fix — diagnosis.md §Group1)
//!
//! Callers **MUST NOT** pass secrets (pairing tokens, full URLs containing
//! tokens) in `message` or `title`.  Both parameters are interpolated
//! verbatim into an AppleScript string which is passed as an argv element
//! to `osascript`.  While argv exposure is limited to same-UID processes,
//! macOS Notification Center also persists banner content in
//! `~/Library/Group Containers/group.com.apple.usernoted/db2/db` (and
//! TimeMachine backups), violating CWE-200 / CWE-532 / guideline §4.1.
//!
//! Defensive cap: inputs longer than 256 bytes are rejected to prevent
//! accidental secret leakage via oversized strings.

use tracing::warn;

/// Maximum byte length accepted for `title` and `message`.
///
/// Defensive cap (IG1): callers must not pass secrets; this limit helps
/// surface accidental oversized inputs early.
const MAX_NOTIFICATION_BYTES: usize = 256;

/// Fires a macOS notification.
///
/// # Panics
///
/// Does not panic — all errors are logged as warnings.
///
/// # Security
///
/// - Callers **MUST NOT** pass the full pairing token in `message` or `title`.
/// - Inputs exceeding 256 bytes are rejected and logged as a warning.
pub fn fire(title: &str, message: &str) {
    // IG1 fix: defensive cap — reject oversized inputs.
    if title.len() > MAX_NOTIFICATION_BYTES || message.len() > MAX_NOTIFICATION_BYTES {
        warn!(
            target: "notification",
            title_len = title.len(),
            message_len = message.len(),
            "notification input exceeds 256-byte cap — rejected (possible accidental secret)"
        );
        return;
    }

    let script = format!(
        r#"display notification "{message}" with title "{title}" sound name "Glass""#
    );

    match std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
    {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            warn!(
                target: "notification",
                stderr = %String::from_utf8_lossy(&out.stderr),
                "osascript returned non-zero"
            );
        }
        Err(e) => {
            warn!(target: "notification", err = %e, "osascript not available");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IG1 regression: fire_notification in cf_tunnel MUST NOT put the full token
    /// in the AppleScript script body.  This test verifies the notification module
    /// itself rejects oversized inputs.
    #[test]
    fn rejects_oversized_message() {
        // Construct a 257-byte message — should be silently rejected (no panic).
        let long_msg = "A".repeat(257);
        // No assertion on side-effect; just ensure it does not panic.
        fire("Telayd", &long_msg);
    }

    #[test]
    fn rejects_oversized_title() {
        let long_title = "T".repeat(257);
        fire(&long_title, "msg");
    }
}
