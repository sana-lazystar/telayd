//! Send-keys mapping for tmux inject.
//!
//! Implements the telayd-protocol §3.3 option-index → keystroke table
//! verbatim, verified by spike 06.1 (FULL PASS single-question).

/// Returns the tmux send-keys arguments (excluding `-t <session>`) for a
/// given option index.
///
/// The returned `Vec<String>` is passed directly to
/// `tokio::process::Command::args()` — never shell-concatenated.
///
/// # Mapping (telayd-protocol §3.3)
/// | choice_index | options_total | keystrokes |
/// |---|---|---|
/// | 1 | any | `["Enter"]` |
/// | 2..=9 | ≤ 9 | `["<digit>", "Enter"]` |
/// | 10+ | > 9 | `Down` × (idx-1) times then `Enter` |
pub fn keystrokes_for(choice_index: u32, options_total: u32) -> Vec<String> {
    match choice_index {
        1 => vec!["Enter".to_string()],
        2..=9 if options_total <= 9 => {
            vec![choice_index.to_string(), "Enter".to_string()]
        }
        idx => {
            // Navigate down from option 1 to option idx: (idx-1) Down presses.
            let mut keys = Vec::with_capacity(idx as usize);
            for _ in 0..(idx - 1) {
                keys.push("Down".to_string());
            }
            keys.push("Enter".to_string());
            keys
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the exact mapping from telayd-protocol §3.3 table.
    #[test]
    fn index_1_any_total_gives_enter() {
        assert_eq!(keystrokes_for(1, 1), vec!["Enter"]);
        assert_eq!(keystrokes_for(1, 5), vec!["Enter"]);
        assert_eq!(keystrokes_for(1, 20), vec!["Enter"]);
    }

    #[test]
    fn index_2_to_9_with_small_total_gives_digit_enter() {
        assert_eq!(keystrokes_for(2, 4), vec!["2", "Enter"]);
        assert_eq!(keystrokes_for(3, 9), vec!["3", "Enter"]);
        assert_eq!(keystrokes_for(9, 9), vec!["9", "Enter"]);
    }

    #[test]
    fn index_10_plus_uses_down_navigation() {
        // index 10 → 9 Down + Enter
        let keys = keystrokes_for(10, 12);
        assert_eq!(keys.len(), 10); // 9 Down + 1 Enter
        for k in &keys[..9] {
            assert_eq!(k, "Down");
        }
        assert_eq!(keys[9], "Enter");
    }

    #[test]
    fn index_2_with_large_total_uses_down_navigation() {
        // When options_total > 9 but choice_index = 2, use Down navigation.
        // (This edge case: the middle branch requires options_total <= 9.)
        let keys = keystrokes_for(2, 10);
        assert_eq!(keys[0], "Down");
        assert_eq!(keys[1], "Enter");
    }
}
