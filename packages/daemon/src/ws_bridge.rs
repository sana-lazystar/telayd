//! WebSocket bridge — axum 0.7 WS server on `ws://127.0.0.1:7777`.
//!
//! Implements:
//! - Pairing handshake (first-frame `pairing-request`, 5s timeout).
//! - 1-client limit (existing connection closed with 4002 on new connect).
//! - Heartbeat: 30s ping, 10s timeout, 3 miss → close.
//! - All 9 message types (ADR-W001, api-spec.md).
//! - Brute-force tarpit: 5 mismatch/IP → 60s delay.
//!
//! Security:
//! - Binds to `127.0.0.1` only (S-3).
//! - Token comparison via `subtle::ConstantTimeEq` (pairing::verify_token).
//! - Envelope max 64 KiB.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use axum::extract::ws::{CloseFrame as AxumCloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::metrics::{LatencyTimer, MetricsCollector};
use crate::protocol::{
    build_frame, Envelope, Inquiry, InquiryAck, InquiryError, InquiryErrorReason,
    InquiryResponse, ModeToggleAck, ModeToggleRequest, PairingAck, PairingReject,
    PairingRejectReason, PermissionMode,
};
use crate::tmux_controller::TmuxController;

// ── Constants ────────────────────────────────────────────────────────────────

const PAIRING_TIMEOUT_SECS: u64 = 5;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_AUTH_FAILURES_PER_IP: u32 = 5;
const TARPIT_DURATION: Duration = Duration::from_secs(60);

// ── Shared daemon state ──────────────────────────────────────────────────────

/// Per-session active client slot.
///
/// IG2 fix: includes an eviction sender so the eviction site can signal
/// the per-connection task to send Close(4002) on its own socket.
struct ActiveClient {
    /// Outbound frame sender (daemon → PWA).
    out_tx: mpsc::Sender<String>,
    /// One-shot eviction signal: sending triggers Close(4002) in the task.
    evict_tx: tokio::sync::oneshot::Sender<()>,
}

/// Shared state injected into every axum handler.
pub struct WsBridgeState {
    pub pairing_token: String,
    /// The currently connected and authenticated client (None when idle).
    /// IG2 fix: stores ActiveClient (out_tx + evict_tx) instead of bare Sender.
    active_client: Mutex<Option<ActiveClient>>,
    /// Pending inquiries: tool_use_id → (Inquiry, Instant start time).
    pub pending_inquiries: RwLock<HashMap<String, (Inquiry, Instant)>>,
    /// Brute-force rate limiter: IP → (failure_count, first_fail_at).
    pub rate_limiter: Mutex<HashMap<String, (u32, Instant)>>,
    pub tmux: Arc<TmuxController>,
    pub metrics: Arc<MetricsCollector>,
    /// Current permission mode (reflects last successful mode-toggle).
    pub permission_mode: Mutex<PermissionMode>,
}

impl WsBridgeState {
    pub fn new(
        pairing_token: String,
        tmux: Arc<TmuxController>,
        metrics: Arc<MetricsCollector>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pairing_token,
            active_client: Mutex::new(None),
            pending_inquiries: RwLock::new(HashMap::new()),
            rate_limiter: Mutex::new(HashMap::new()),
            tmux,
            metrics,
            permission_mode: Mutex::new(PermissionMode::Default),
        })
    }
}

// ── axum router ──────────────────────────────────────────────────────────────

/// Builds and binds the axum router on `127.0.0.1:7777`.
///
/// Runs until `cancellation_token` is cancelled.
pub async fn run_ws_bridge(
    state: Arc<WsBridgeState>,
    cancellation_token: Arc<tokio_util::sync::CancellationToken>,
) -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:7777".parse()?;

    let app = Router::new()
        .route("/", get(ws_upgrade_handler))
        .route("/health", get(health_handler))
        .with_state(state.clone());

    info!(target: "ws_bridge", %addr, "WS bridge listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        cancellation_token.cancelled().await;
        info!(target: "ws_bridge", "WS bridge shutting down");
    })
    .await?;

    Ok(())
}

/// Forwards an `Inquiry` to the connected PWA client (if any).
///
/// IG2 fix (diagnosis.md §Group2 P1): before pushing the inquiry, if the
/// stored `permission_mode` differs from the mode carried in the inquiry payload
/// (or any mode toggle was deferred), replay `apply_permission_mode` once so that
/// idle-state toggles take effect on the first inquiry after the toggle.
pub async fn push_inquiry(state: &Arc<WsBridgeState>, inquiry: Inquiry) -> Result<()> {
    // IG2 fix: replay deferred mode toggle on inquiry arrival.
    // Only send the mode command if we have an active session to target.
    {
        let current_mode = state.permission_mode.lock().await.clone();
        // Use the inquiry's tmux_session as the target for mode replay.
        let sess = inquiry.tmux_session.clone();
        if !sess.is_empty() {
            // Check whether the session is registered — if so, apply mode idempotently.
            if state.tmux.first_active_session().is_some() {
                // Idempotent replay: apply the current stored mode to this session.
                // This ensures that a deferred toggle (applied=false due to no-session)
                // takes effect when the next inquiry arrives.
                let mode_applied = state.tmux.send_mode_command(&sess, &current_mode).await;
                if mode_applied {
                    debug!(
                        target: "ws_bridge",
                        tmux_session = %sess,
                        mode = ?current_mode,
                        "replayed deferred permission mode on inquiry-push"
                    );
                }
            }
        }
    }

    let client = state.active_client.lock().await;
    if let Some(ac) = &*client {
        // Register in pending map.
        {
            let mut pending = state.pending_inquiries.write().await;
            pending.insert(inquiry.tool_use_id.clone(), (inquiry.clone(), Instant::now()));
        }
        let frame = build_frame("inquiry-push", &inquiry.tool_use_id, &inquiry)?;
        ac.out_tx.send(frame).await.map_err(|_| anyhow::anyhow!("client channel closed"))?;
    } else {
        warn!(
            target: "ws_bridge",
            tool_use_id = %inquiry.tool_use_id,
            "inquiry-push dropped — no connected client"
        );
    }
    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn health_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({ "status": "ok", "v": 1 }))
}

async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<WsBridgeState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, addr, state))
}

// ── Per-connection handler ────────────────────────────────────────────────────

async fn handle_ws(mut socket: WebSocket, addr: SocketAddr, state: Arc<WsBridgeState>) {
    let ip = addr.ip().to_string();
    info!(target: "ws_bridge", %ip, "new WS connection");

    // Check tarpit before doing anything.
    if is_tarpitted(&state, &ip).await {
        warn!(target: "ws_bridge", %ip, "connection rejected — IP tarpitted");
        return;
    }

    // ── 5-second pairing timeout ──────────────────────────────────────────
    let first_msg = tokio::time::timeout(
        Duration::from_secs(PAIRING_TIMEOUT_SECS),
        socket.recv(),
    )
    .await;

    let raw = match first_msg {
        Ok(Some(Ok(Message::Text(t)))) => t,
        _ => {
            let _ = socket
                .send(Message::Close(Some(AxumCloseFrame {
                    code: 4001,
                    reason: "expected pairing-request".into(),
                })))
                .await;
            return;
        }
    };

    if raw.len() > MAX_FRAME_BYTES {
        let _ = socket
            .send(Message::Close(Some(AxumCloseFrame {
                code: 1009,
                reason: "message too big".into(),
            })))
            .await;
        return;
    }

    // Parse envelope.
    // IG1 fix: emit pairing-reject frame on ALL 4001 paths (not just token-mismatch).
    // IG2 fix: pairing-reject frames now use PairingRejectReason typed enum.
    let env: Envelope = match serde_json::from_str(&raw) {
        Ok(e) => e,
        Err(_) => {
            record_auth_failure(&state, &ip).await;
            // Emit reject frame — correlation id unknown, use placeholder.
            let _ = send_pairing_reject(&mut socket, "unknown", PairingRejectReason::BadEnvelope).await;
            let _ = socket
                .send(Message::Close(Some(AxumCloseFrame {
                    code: 4001,
                    reason: "bad-envelope".into(),
                })))
                .await;
            return;
        }
    };

    if env.v != 1 {
        record_auth_failure(&state, &ip).await;
        let _ = send_pairing_reject(&mut socket, &env.id, PairingRejectReason::UnsupportedVersion).await;
        let _ = socket
            .send(Message::Close(Some(AxumCloseFrame {
                code: 4001,
                reason: "unsupported-version".into(),
            })))
            .await;
        return;
    }

    if env.msg_type != "pairing-request" {
        record_auth_failure(&state, &ip).await;
        let _ = send_pairing_reject(&mut socket, &env.id, PairingRejectReason::BadEnvelope).await;
        let _ = socket
            .send(Message::Close(Some(AxumCloseFrame {
                code: 4001,
                reason: "expected pairing-request".into(),
            })))
            .await;
        return;
    }

    let req: crate::protocol::PairingRequest = match serde_json::from_value(env.payload.clone()) {
        Ok(r) => r,
        Err(_) => {
            record_auth_failure(&state, &ip).await;
            let _ = send_pairing_reject(&mut socket, &env.id, PairingRejectReason::BadPayload).await;
            let _ = socket
                .send(Message::Close(Some(AxumCloseFrame {
                    code: 4001,
                    reason: "bad-payload".into(),
                })))
                .await;
            return;
        }
    };

    // Token format validation (never leak timing on format-invalid tokens).
    if !crate::pairing::is_valid_token_format(&req.token) {
        record_auth_failure(&state, &ip).await;
        let _ = send_pairing_reject(&mut socket, &env.id, PairingRejectReason::TokenMismatch).await;
        let _ = socket
            .send(Message::Close(Some(AxumCloseFrame {
                code: 4001,
                reason: "token-mismatch".into(),
            })))
            .await;
        return;
    }

    // Check tarpit again (may have been updated by other calls).
    if is_tarpitted(&state, &ip).await {
        let _ = send_pairing_reject(&mut socket, &env.id, PairingRejectReason::TokenMismatch).await;
        let _ = socket
            .send(Message::Close(Some(AxumCloseFrame {
                code: 4001,
                reason: "rate-limited".into(),
            })))
            .await;
        return;
    }

    // Constant-time token comparison.
    if !crate::pairing::verify_token(&state.pairing_token, &req.token) {
        warn!(target: "auth", remote_ip = %ip, reason = "token_mismatch", "pairing rejected");
        record_auth_failure(&state, &ip).await;

        // Send pairing-reject before closing (api-spec §pairing-reject).
        let _ = send_pairing_reject(&mut socket, &env.id, PairingRejectReason::TokenMismatch).await;
        let _ = socket
            .send(Message::Close(Some(AxumCloseFrame {
                code: 4001,
                reason: "token-mismatch".into(),
            })))
            .await;
        return;
    }

    // ── Evict existing client (1-client limit) ──────────────────────────────
    // IG2 fix: fire evict_tx so the old task sends Close(4002,"replaced") on its own socket.
    {
        let mut active = state.active_client.lock().await;
        if let Some(old) = active.take() {
            info!(target: "ws_bridge", %ip, "evicting existing client (close 4002)");
            // Sending on evict_tx signals the old event loop to emit Close(4002).
            // If it's already gone, the error is harmless.
            let _ = old.evict_tx.send(());
        }
    }

    // ── Pairing success ─────────────────────────────────────────────────────
    let session_id = Uuid::new_v4().to_string();
    if let Ok(ack_frame) = build_frame(
        "pairing-ack",
        &env.id,
        &PairingAck {
            session: format!("sess_{session_id}"),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    ) {
        if socket.send(Message::Text(ack_frame.into())).await.is_err() {
            return;
        }
    }

    info!(target: "auth", remote_ip = %ip, session = %session_id, "pairing successful");

    // ── Outbound channel + per-session eviction channel ──────────────────────
    // IG2 fix: per-task oneshot eviction channel; eviction site fires evict_tx.
    let (out_tx, mut out_rx) = mpsc::channel::<String>(64);
    let (evict_tx, mut evict_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut active = state.active_client.lock().await;
        *active = Some(ActiveClient { out_tx, evict_tx });
    }

    // ── Event loop ─────────────────────────────────────────────────────────
    let mut missed_pings: u32 = 0;
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    let mut heartbeat_timer: Option<tokio::time::Instant> = None;
    // Tracks whether this task was evicted (determines close code on exit).
    let mut evicted = false;

    loop {
        tokio::select! {
            // Eviction signal from a new connecting client (IG2 fix).
            _ = &mut evict_rx => {
                info!(target: "ws_bridge", %ip, "evicted by new client — sending close 4002");
                let _ = socket.send(Message::Close(Some(AxumCloseFrame {
                    code: 4002,
                    reason: "replaced".into(),
                }))).await;
                evicted = true;
                break;
            }

            // Outbound frame from daemon.
            Some(frame) = out_rx.recv() => {
                if socket.send(Message::Text(frame.into())).await.is_err() {
                    break;
                }
            }

            // Inbound frame from PWA.
            msg_result = socket.recv() => {
                match msg_result {
                    Some(Ok(Message::Text(raw_msg))) => {
                        if raw_msg.len() > MAX_FRAME_BYTES {
                            let _ = socket.send(Message::Close(Some(AxumCloseFrame {
                                code: 1009,
                                reason: "message too big".into(),
                            }))).await;
                            break;
                        }
                        if let Err(e) = handle_inbound(&raw_msg, &state, &session_id).await {
                            warn!(target: "ws_bridge", err = %e, "inbound message error");
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        debug!(target: "ws_bridge", "pong received");
                        missed_pings = 0;
                        heartbeat_timer = None;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!(target: "ws_bridge", %ip, "client closed");
                        break;
                    }
                    Some(Err(e)) => {
                        warn!(target: "ws_bridge", %ip, err = %e, "WS error");
                        break;
                    }
                    _ => {}
                }
            }

            // Heartbeat tick.
            _ = heartbeat.tick() => {
                if let Some(t) = heartbeat_timer {
                    if t.elapsed() > HEARTBEAT_TIMEOUT {
                        missed_pings += 1;
                        warn!(target: "ws_bridge", %ip, missed_pings, "heartbeat timeout");
                        if missed_pings >= 3 {
                            let _ = socket.send(Message::Close(Some(AxumCloseFrame {
                                code: 1001,
                                reason: "heartbeat timeout".into(),
                            }))).await;
                            break;
                        }
                    }
                }
                heartbeat_timer = Some(tokio::time::Instant::now());
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
        }
    }

    // Clean up active client slot (only if not already replaced by the new client).
    if !evicted {
        let mut active = state.active_client.lock().await;
        if active.is_some() {
            *active = None;
        }
    }
    info!(target: "ws_bridge", %ip, "connection closed");
}

// ── Inbound message dispatch ──────────────────────────────────────────────────

async fn handle_inbound(
    raw: &str,
    state: &Arc<WsBridgeState>,
    _session_id: &str,
) -> Result<()> {
    let env: Envelope = serde_json::from_str(raw).map_err(|e| anyhow::anyhow!("envelope parse: {e}"))?;

    if env.v != 1 {
        anyhow::bail!("unsupported protocol version {}", env.v);
    }

    if !crate::protocol::ALLOWED_TYPES.contains(&env.msg_type.as_str()) {
        anyhow::bail!("unknown message type: {}", env.msg_type);
    }

    match env.msg_type.as_str() {
        "inquiry-response" => {
            handle_inquiry_response(env, state).await?;
        }
        "mode-toggle-request" => {
            handle_mode_toggle(env, state).await?;
        }
        _ => {
            // Client sent a server-originated type — ignore gracefully.
            debug!(target: "ws_bridge", msg_type = %env.msg_type, "ignored client-sent type");
        }
    }
    Ok(())
}

async fn handle_inquiry_response(env: Envelope, state: &Arc<WsBridgeState>) -> Result<()> {
    // IG1 fix: parse payload; on failure emit inquiry-error{validation} before returning.
    let resp: InquiryResponse = match serde_json::from_value(env.payload) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "ws_bridge", err = %e, "inquiry-response parse failed");
            // tool_use_id may be the correlation id from the envelope.
            let tool_use_id = env.id.clone();
            let err_frame = build_frame(
                "inquiry-error",
                &tool_use_id,
                &InquiryError {
                    tool_use_id: tool_use_id.clone(),
                    reason: InquiryErrorReason::Validation,
                },
            )?;
            if let Some(ac) = &*state.active_client.lock().await {
                let _ = ac.out_tx.send(err_frame).await;
            }
            anyhow::bail!("inquiry-response parse: {e}");
        }
    };

    // Look up pending inquiry to get options_total.
    // IG1 fix: emit inquiry-error{inquiry-stale} before returning.
    let (inquiry, start_time) = {
        let pending = state.pending_inquiries.read().await;
        match pending.get(&resp.tool_use_id).cloned() {
            Some(entry) => entry,
            None => {
                warn!(
                    target: "ws_bridge",
                    tool_use_id = %resp.tool_use_id,
                    "inquiry-stale"
                );
                let err_frame = build_frame(
                    "inquiry-error",
                    &resp.tool_use_id,
                    &InquiryError {
                        tool_use_id: resp.tool_use_id.clone(),
                        reason: InquiryErrorReason::InquiryStale,
                    },
                )?;
                if let Some(ac) = &*state.active_client.lock().await {
                    let _ = ac.out_tx.send(err_frame).await;
                }
                anyhow::bail!("inquiry-stale: {}", resp.tool_use_id);
            }
        }
    };

    let options_total = inquiry
        .questions
        .first()
        .map(|q| q.options.len() as u32)
        .unwrap_or(1);

    // IG1 fix: emit inquiry-error{validation} on validate failure.
    if let Err(e) = resp.validate(options_total) {
        warn!(
            target: "ws_bridge",
            tool_use_id = %resp.tool_use_id,
            err = %e,
            "inquiry-response validation failed"
        );
        let err_frame = build_frame(
            "inquiry-error",
            &resp.tool_use_id,
            &InquiryError {
                tool_use_id: resp.tool_use_id.clone(),
                reason: InquiryErrorReason::Validation,
            },
        )?;
        if let Some(ac) = &*state.active_client.lock().await {
            let _ = ac.out_tx.send(err_frame).await;
        }
        return Err(e);
    }

    let timer = LatencyTimer::start();

    // Resolve inject parameters.
    let (choice_index, free_text) = match (&resp.choice_index, &resp.free_text, &resp.cancel) {
        (Some(idx), _, _) => (*idx, None),
        (_, Some(text), _) => (0, Some(text.as_str())),
        _ => {
            // Cancel — send inquiry-ack with latency 0.
            let latency_ms = start_time.elapsed().as_millis() as u64;
            let ack = build_frame(
                "inquiry-ack",
                &resp.tool_use_id,
                &InquiryAck {
                    tool_use_id: resp.tool_use_id.clone(),
                    latency_ms,
                },
            )?;
            if let Some(ac) = &*state.active_client.lock().await {
                let _ = ac.out_tx.send(ack).await;
            }
            let mut pending = state.pending_inquiries.write().await;
            pending.remove(&resp.tool_use_id);
            return Ok(());
        }
    };

    // Check multi-question (L0: single-question verified only).
    if inquiry.questions.len() > 1 {
        warn!(
            target: "ws_bridge",
            tool_use_id = %resp.tool_use_id,
            "multi-question inquiry — single inject, manual fallback may be needed"
        );
    }

    // Perform tmux inject.
    let inject_result = state
        .tmux
        .inject(
            &inquiry.tmux_session,
            choice_index,
            options_total,
            free_text,
            &resp.tool_use_id,
        )
        .await;

    let latency_ms = timer.elapsed_ms();
    state.metrics.record_latency(latency_ms);

    // Remove from pending.
    {
        let mut pending = state.pending_inquiries.write().await;
        pending.remove(&resp.tool_use_id);
    }

    let client = state.active_client.lock().await;
    match inject_result {
        Ok(()) => {
            let ack = build_frame(
                "inquiry-ack",
                &resp.tool_use_id,
                &InquiryAck {
                    tool_use_id: resp.tool_use_id.clone(),
                    latency_ms,
                },
            )?;
            if let Some(ac) = &*client {
                let _ = ac.out_tx.send(ack).await;
            }
            // IG11 fix: structured log shape matching test-strategy §3.1 SSOT.
            info!(
                target: "metrics",
                event = "inject",
                tool_use_id = %resp.tool_use_id,
                latency_ms,
                "inject success"
            );
        }
        // IG1 fix: inject failure emits inquiry-error{send-keys-failed} (typed enum).
        Err(e) => {
            warn!(
                target: "ws_bridge",
                tool_use_id = %resp.tool_use_id,
                err = %e,
                "inject failed"
            );
            let err_frame = build_frame(
                "inquiry-error",
                &resp.tool_use_id,
                &InquiryError {
                    tool_use_id: resp.tool_use_id.clone(),
                    reason: InquiryErrorReason::SendKeysFailed,
                },
            )?;
            if let Some(ac) = &*client {
                let _ = ac.out_tx.send(err_frame).await;
            }
        }
    }
    Ok(())
}

async fn handle_mode_toggle(env: Envelope, state: &Arc<WsBridgeState>) -> Result<()> {
    let req: ModeToggleRequest = serde_json::from_value(env.payload)
        .map_err(|e| anyhow::anyhow!("mode-toggle-request parse: {e}"))?;

    info!(target: "ws_bridge", mode = ?req.mode, "mode-toggle-request");

    // IG2 fix: apply_permission_mode now returns (applied, reason).
    let (applied, reason) = apply_permission_mode(&req.mode, state).await;

    // IG2 fix: always persist the requested mode (whether applied immediately or deferred).
    // On next inquiry-push the mode will be replayed.
    {
        let mut current = state.permission_mode.lock().await;
        *current = req.mode.clone();
    }

    let ack = build_frame(
        "mode-toggle-ack",
        &env.id,
        &ModeToggleAck {
            mode: req.mode,
            applied,
            reason: reason.map(str::to_owned),
        },
    )?;

    let client = state.active_client.lock().await;
    if let Some(ac) = &*client {
        let _ = ac.out_tx.send(ack).await;
    }
    Ok(())
}

/// Applies a permission mode toggle by writing `/mode <name>` to the tmux pane.
///
/// Claude Code accepts `/mode plan`, `/mode accept-edits`, `/mode default`
/// (Anthropic #35637 wedge).
///
/// IG2 fix (diagnosis.md §Group2 P1):
/// - Falls back to `tmux.active_sessions` when `pending_inquiries` is empty
///   (covers the idle-state toggle scenario from Anthropic #35637).
/// - If still no session is known, returns `(false, "no-session")`.
///
/// IG6 fix: routes through `TmuxController::send_mode_command` which validates
/// the session against the active whitelist — no direct `Command::new("tmux")`.
async fn apply_permission_mode(mode: &PermissionMode, state: &Arc<WsBridgeState>) -> (bool, Option<&'static str>) {
    // First: try pending inquiries (mid-prompt case).
    let session = {
        let pending = state.pending_inquiries.read().await;
        pending.values().next().map(|(inq, _)| inq.tmux_session.clone())
    };

    // IG2 fix: Fall back to active_sessions if no pending inquiry.
    let session = if let Some(s) = session {
        Some(s)
    } else {
        // TmuxController::active_sessions() returns the list of registered sessions.
        state.tmux.first_active_session()
    };

    if let Some(sess) = session {
        let applied = state.tmux.send_mode_command(&sess, mode).await;
        (applied, None)
    } else {
        // No active session known — deferred apply (mode is stored, replayed on next inquiry).
        warn!(target: "ws_bridge", "mode-toggle: no active session — deferring until next inquiry-push");
        (false, Some("no-session"))
    }
}

// ── Helper: send pairing-reject frame ────────────────────────────────────────

/// Sends a `pairing-reject` frame on the socket.
///
/// IG1/IG2 fix: every 4001 close path emits a typed-reason reject frame
/// before the Close frame so the PWA can display the correct error message.
async fn send_pairing_reject(
    socket: &mut WebSocket,
    correlation_id: &str,
    reason: PairingRejectReason,
) -> Result<()> {
    let frame = build_frame("pairing-reject", correlation_id, &PairingReject { reason })?;
    socket.send(Message::Text(frame.into())).await?;
    Ok(())
}

// ── Rate limiter ──────────────────────────────────────────────────────────────

async fn record_auth_failure(state: &Arc<WsBridgeState>, ip: &str) {
    let mut rl = state.rate_limiter.lock().await;
    let entry = rl.entry(ip.to_string()).or_insert((0, Instant::now()));
    entry.0 += 1;
    if entry.0 == 1 {
        entry.1 = Instant::now();
    }
}

async fn is_tarpitted(state: &Arc<WsBridgeState>, ip: &str) -> bool {
    let mut rl = state.rate_limiter.lock().await;
    if let Some((count, first_fail)) = rl.get(ip) {
        if *count >= MAX_AUTH_FAILURES_PER_IP {
            if first_fail.elapsed() < TARPIT_DURATION {
                return true;
            }
            // Reset after tarpit window.
            rl.remove(ip);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{InquiryOption, InquiryQuestion};

    fn make_state() -> Arc<WsBridgeState> {
        let tmux = Arc::new(TmuxController::new(Arc::new(
            crate::sentinel::NoopSentinelParser,
        )));
        let metrics = Arc::new(MetricsCollector::new());
        WsBridgeState::new("A".repeat(43), tmux, metrics)
    }

    fn make_inquiry(tool_use_id: &str, session: &str) -> Inquiry {
        Inquiry {
            kind: "inquiry".to_string(),
            tool_use_id: tool_use_id.to_string(),
            session_id: "sess".to_string(),
            tmux_session: session.to_string(),
            header: "Test".to_string(),
            questions: vec![InquiryQuestion {
                question: "Choose".to_string(),
                options: vec![
                    InquiryOption { index: 1, label: "A".to_string(), description: "".to_string() },
                    InquiryOption { index: 2, label: "B".to_string(), description: "".to_string() },
                ],
                multi_select: false,
            }],
            permission_mode: None,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn pairing_token_verify_same() {
        let token = crate::pairing::generate_token().unwrap();
        assert!(crate::pairing::verify_token(&token, &token));
    }

    #[test]
    fn pairing_token_verify_mismatch() {
        let a = crate::pairing::generate_token().unwrap();
        let b = crate::pairing::generate_token().unwrap();
        assert!(!crate::pairing::verify_token(&a, &b));
    }

    #[tokio::test]
    async fn rate_limiter_tarpits_after_5_failures() {
        let state = make_state();
        let ip = "1.2.3.4";
        for _ in 0..5 {
            record_auth_failure(&state, ip).await;
        }
        assert!(is_tarpitted(&state, ip).await);
    }

    #[tokio::test]
    async fn inquiry_response_cancel_cleans_up_pending() {
        let state = make_state();
        let inq = make_inquiry("toolu_TEST123456789012345678901234", "my-session");
        {
            let mut pending = state.pending_inquiries.write().await;
            pending.insert(
                inq.tool_use_id.clone(),
                (inq.clone(), Instant::now()),
            );
        }

        // Simulate cancel response.
        let resp_json = serde_json::json!({
            "v": 1,
            "type": "inquiry-response",
            "id": &inq.tool_use_id,
            "ts": "2026-01-01T00:00:00.000Z",
            "payload": {
                "tool_use_id": &inq.tool_use_id,
                "cancel": true
            }
        });
        let raw = serde_json::to_string(&resp_json).unwrap();
        let _ = handle_inbound(&raw, &state, "sess-123").await;

        let pending = state.pending_inquiries.read().await;
        assert!(!pending.contains_key(&inq.tool_use_id));
    }
}
