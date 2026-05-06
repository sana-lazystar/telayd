//! Pairing token generation, storage, and verification.
//!
//! Security requirements (scope §3.4 S-1/S-2):
//! - 32-byte (256-bit) entropy from `getrandom` (`/dev/urandom`).
//! - RFC 4648 base64url **no-padding** → exactly 43 ASCII chars.
//! - Stored in `~/.config/telayd/config.toml` with mode 0600.
//! - `verify_token` uses constant-time comparison via `subtle::ConstantTimeEq`.
//! - `PairingToken` `Debug`/`Display` mask all but the first 4 chars.

use base64::Engine as _;
use subtle::ConstantTimeEq;

/// Opaque pairing token wrapper.
///
/// Never derives `Debug` or `Display` automatically — both are manually
/// implemented to avoid accidental logging of the full token.
pub struct PairingToken(String);

impl std::fmt::Debug for PairingToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = &self.0[..4.min(self.0.len())];
        write!(f, "PairingToken({prefix}***)")
    }
}

impl std::fmt::Display for PairingToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = &self.0[..4.min(self.0.len())];
        write!(f, "{prefix}***")
    }
}

impl PairingToken {
    /// Wraps an already-validated 43-char base64url token string.
    pub fn new(raw: String) -> anyhow::Result<Self> {
        if !is_valid_token_format(&raw) {
            anyhow::bail!("invalid token format (expected 43-char base64url no-padding)");
        }
        Ok(Self(raw))
    }

    /// Generates a fresh cryptographically-random token.
    pub fn generate() -> anyhow::Result<Self> {
        Ok(Self(generate_token()?))
    }

    /// Exposes the raw token string for comparison or config serialisation.
    ///
    /// Intentionally named `expose_secret` so callers are grep-able.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Constant-time equality check against a presented string.
    #[must_use]
    pub fn verify(&self, presented: &str) -> bool {
        verify_token(self.expose_secret(), presented)
    }
}

// ── Low-level functions (also used by security-guidelines example) ──────────

/// Generates a 32-byte random token encoded as RFC 4648 base64url no-padding.
pub fn generate_token() -> anyhow::Result<String> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf)
        .map_err(|e| anyhow::anyhow!("getrandom failed: {e}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

/// Returns `true` iff `s` is exactly 43 chars of `[A-Za-z0-9_-]`.
pub fn is_valid_token_format(s: &str) -> bool {
    s.len() == 43
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Constant-time equality comparison.
///
/// Length is not secret (token length is fixed at 43), so the early-exit
/// on length mismatch does not leak useful timing information.
#[must_use]
pub fn verify_token(stored: &str, presented: &str) -> bool {
    if stored.len() != presented.len() {
        return false;
    }
    stored.as_bytes().ct_eq(presented.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn token_format_43_chars_base64url() {
        let token = generate_token().unwrap();
        assert_eq!(token.len(), 43, "token must be 43 chars");
        assert!(
            is_valid_token_format(&token),
            "must match ^[A-Za-z0-9_-]{{43}}$"
        );
    }

    #[test]
    fn token_uniqueness_1000_samples() {
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let t = generate_token().unwrap();
            assert!(!seen.contains(&t), "collision detected!");
            seen.insert(t);
        }
    }

    #[test]
    fn verify_token_constant_time_equal() {
        let stored = generate_token().unwrap();
        assert!(verify_token(&stored, &stored));
    }

    #[test]
    fn verify_token_mismatch() {
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        // Two independently-generated tokens should not match.
        // Probability of false positive: 2^-256 ≈ 0.
        assert!(!verify_token(&a, &b));
    }

    #[test]
    fn verify_token_length_mismatch() {
        let stored = generate_token().unwrap();
        assert!(!verify_token(&stored, &stored[..42]));
    }

    #[test]
    fn is_valid_format_rejects_bad_chars() {
        assert!(!is_valid_token_format("!nv@lid_chars_here_padding==padding"));
    }

    #[test]
    fn is_valid_format_rejects_wrong_length() {
        assert!(!is_valid_token_format("short"));
        assert!(!is_valid_token_format(
            "toolongAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        ));
    }

    #[test]
    fn pairing_token_debug_masks() {
        let t = PairingToken::generate().unwrap();
        let debug = format!("{t:?}");
        assert!(debug.contains("***"), "debug should mask token");
        // Must not contain the full raw token.
        assert!(!debug.contains(t.expose_secret()));
    }

    #[test]
    fn pairing_token_display_masks() {
        let t = PairingToken::generate().unwrap();
        let display = format!("{t}");
        assert!(display.contains("***"));
        assert!(!display.contains(t.expose_secret()));
    }
}
