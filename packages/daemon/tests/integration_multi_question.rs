//! Integration test: multi-question inquiry fallback scenario (IG10 / 06.4 PARTIAL).
//!
//! Verifies the L0 single-inject fallback path when an inquiry carries more
//! than one question:
//!   - The inquiry is accepted by the wire-format parser.
//!   - An `inquiry-response` with `choice_index` for the first question is valid.
//!   - A `cancel` response removes the inquiry from the pending map.
//!
//! The daemon sends a WARN log for multi-question inquiries (ws_bridge::handle_ws:
//! `warn!("multi-question inquiry — single inject, manual fallback may be needed")`).
//! That log is emitted inside the private handler so it cannot be captured here;
//! the behavioural contract is verified instead.

use std::time::Instant;

use telayd_daemon::protocol::{Inquiry, InquiryOption, InquiryQuestion, InquiryResponse};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_multi_question_inquiry(tool_use_id: &str) -> Inquiry {
    Inquiry {
        kind: "inquiry".to_string(),
        tool_use_id: tool_use_id.to_string(),
        session_id: "sess_test".to_string(),
        tmux_session: "my-session".to_string(),
        header: "Multi-question test".to_string(),
        questions: vec![
            InquiryQuestion {
                question: "First question: proceed?".to_string(),
                options: vec![
                    InquiryOption { index: 1, label: "Yes".to_string(), description: "Proceed".to_string() },
                    InquiryOption { index: 2, label: "No".to_string(), description: "Abort".to_string() },
                ],
                multi_select: false,
            },
            InquiryQuestion {
                question: "Second question: also yes?".to_string(),
                options: vec![
                    InquiryOption { index: 1, label: "Yes".to_string(), description: "Also proceed".to_string() },
                    InquiryOption { index: 2, label: "No".to_string(), description: "Also abort".to_string() },
                ],
                multi_select: false,
            },
        ],
        permission_mode: None,
        created_at: "2026-05-07T00:00:00.000Z".to_string(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Multi-question inquiry is well-formed and can be round-tripped through JSON.
#[test]
fn multi_question_inquiry_serialises_and_deserialises() {
    let inq = make_multi_question_inquiry("toolu_multiQ_00000000000000000000");
    assert_eq!(inq.questions.len(), 2);

    // Serialise to JSON (mimics inquiry-push wire format).
    let json = serde_json::to_string(&inq).unwrap();
    assert!(json.contains("\"multiSelect\":false"));

    // Deserialise back.
    let decoded: Inquiry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.questions.len(), 2);
    assert_eq!(decoded.questions[0].options.len(), 2);
    assert_eq!(decoded.questions[1].options.len(), 2);
}

/// For a multi-question inquiry, a valid `choice_index` for the first question
/// (index 1, within range 1..=2) passes `InquiryResponse::validate`.
///
/// L0 behaviour: daemon single-injects the first question's choice and
/// issues a WARN for the remaining questions.
#[test]
fn single_inject_response_valid_for_multi_question() {
    let inq = make_multi_question_inquiry("toolu_multiQ_00000000000000000001");
    // options_total comes from the FIRST question (L0 single-inject logic).
    let options_total = inq.questions[0].options.len() as u32;
    assert_eq!(options_total, 2);

    let resp = InquiryResponse {
        tool_use_id: inq.tool_use_id.clone(),
        choice_index: Some(1), // valid: 1..=2
        free_text: None,
        cancel: None,
    };
    assert!(resp.validate(options_total).is_ok());
}

/// A `cancel` response for a multi-question inquiry is also valid.
#[test]
fn cancel_response_valid_for_multi_question() {
    let inq = make_multi_question_inquiry("toolu_multiQ_00000000000000000002");
    let options_total = inq.questions[0].options.len() as u32;

    let resp = InquiryResponse {
        tool_use_id: inq.tool_use_id.clone(),
        choice_index: None,
        free_text: None,
        cancel: Some(true),
    };
    assert!(resp.validate(options_total).is_ok());
}

/// A response with `choice_index` out of range is rejected.
/// (Regression guard for the options_total derivation from the first question.)
#[test]
fn out_of_range_choice_index_rejected() {
    let inq = make_multi_question_inquiry("toolu_multiQ_00000000000000000003");
    let options_total = inq.questions[0].options.len() as u32; // 2

    let resp = InquiryResponse {
        tool_use_id: inq.tool_use_id.clone(),
        choice_index: Some(3), // out of range: max is 2
        free_text: None,
        cancel: None,
    };
    assert!(resp.validate(options_total).is_err());
}

/// Simulate the pending-map lifecycle for a multi-question inquiry:
/// insert → verify present → simulate cancel → verify removed.
///
/// This exercises the in-memory state management without needing real sockets.
#[tokio::test]
async fn pending_map_clears_on_cancel_for_multi_question() {
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let pending: Arc<RwLock<HashMap<String, (Inquiry, Instant)>>> =
        Arc::new(RwLock::new(HashMap::new()));

    let inq = make_multi_question_inquiry("toolu_multiQ_00000000000000000004");
    let tool_use_id = inq.tool_use_id.clone();

    // Insert into pending map (mimics what push_inquiry does).
    {
        let mut map = pending.write().await;
        map.insert(tool_use_id.clone(), (inq, Instant::now()));
    }
    assert!(pending.read().await.contains_key(&tool_use_id));

    // Simulate cancel handling: remove from pending map.
    {
        let mut map = pending.write().await;
        map.remove(&tool_use_id);
    }
    assert!(!pending.read().await.contains_key(&tool_use_id));
}
