//! Sentinel Parser — L0 stub.
//!
//! Reserves the extensibility slot for L1+ hybrid protocol support.
//!
//! L1+ will introduce `StatusSentinelParser` that detects
//! `<<TELAYD|status|msg=<text>|pct=<0-100>>>` patterns emitted by
//! the LLM and forwards them as `status-push` WS frames.
//!
//! L0 format (reserved, not parsed):
//! `<<TELAYD|status|msg=<text>|pct=<0-100>>>`

/// A parsed sentinel status event (L1+ — unused in L0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEvent {
    /// Human-readable status message.
    pub message: String,
    /// Progress percentage 0..=100.
    pub percent: u8,
}

/// Parses lines of tmux pipe-pane output looking for sentinel markers.
///
/// Implementations must be `Send + Sync` to be held in `Arc<>`.
pub trait SentinelParser: Send + Sync {
    /// Returns `Some(StatusEvent)` if `line` contains a valid sentinel.
    /// Returns `None` for all non-sentinel lines.
    fn parse_line(&self, line: &str) -> Option<StatusEvent>;
}

/// L0 no-op implementation — always returns `None`.
pub struct NoopSentinelParser;

impl SentinelParser for NoopSentinelParser {
    fn parse_line(&self, _line: &str) -> Option<StatusEvent> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_returns_none_for_any_input() {
        let parser = NoopSentinelParser;
        assert!(parser.parse_line("").is_none());
        assert!(parser.parse_line("<<TELAYD|status|msg=hello|pct=50>>").is_none());
        assert!(parser.parse_line("normal tmux output line").is_none());
        assert!(parser.parse_line("<<TELAYD|status|msg=done|pct=100>>").is_none());
    }

    /// Validates the trait shape with a mock implementation (P2).
    #[test]
    fn mock_parser_trait_shape() {
        struct MockStatusParser;
        impl SentinelParser for MockStatusParser {
            fn parse_line(&self, line: &str) -> Option<StatusEvent> {
                if line.contains("<<TELAYD") {
                    Some(StatusEvent {
                        message: "mock".to_string(),
                        percent: 42,
                    })
                } else {
                    None
                }
            }
        }

        // Verify DI pattern works (Box<dyn SentinelParser>).
        let parser: Box<dyn SentinelParser> = Box::new(MockStatusParser);
        assert!(parser.parse_line("plain text").is_none());
        let event = parser.parse_line("<<TELAYD|status|msg=x|pct=42>>");
        assert!(event.is_some());
        assert_eq!(event.unwrap().percent, 42);
    }
}
