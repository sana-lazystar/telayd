//! macOS notification wrapper (osascript).
//!
//! Fires a native macOS notification with title "Telayd".
//! Failure is non-fatal (user may have disabled notifications).

use tracing::warn;

/// Fires a macOS notification.
///
/// `message` must not contain the full pairing token —
/// caller is responsible for masking.
pub fn fire(title: &str, message: &str) {
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
