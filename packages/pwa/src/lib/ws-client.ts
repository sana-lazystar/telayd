/**
 * WebSocket Client (FE-pwa-3)
 * - native WebSocket API
 * - exponential backoff reconnect (architecture.md §2.5)
 * - visibilitychange foreground hook
 * - envelope serialize/deserialize (protocol.ts)
 * Security: token never logged, envelope schema validated
 *
 * Fix: IG1 — delete heartbeat sender (daemon Ping/Pong covers it); per-type narrowing
 * Fix: IG2 — handle close 4002 (replaced); do not reconnect
 * Fix: IG3 — add 'disconnected' to WsStatus; call _reconnect.start() on unplanned close
 * Fix: IG4 — subscribeAck / subscribeError API replacing global callback swap pattern
 */

import {
  type Envelope,
  type InquiryAckPayload,
  type InquiryErrorPayload,
  type InquiryPushPayload,
  type ModeToggleAckPayload,
  type PairingAckPayload,
  type PairingRejectPayload,
  makeEnvelope,
  parseEnvelope,
} from './protocol'
import { ReconnectScheduler } from './reconnect'

// IG3: add 'disconnected' to WsStatus union (was missing — cast as WsStatus at line 127)
export type WsStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'disconnected'
  | 'pairing-error'

export interface WsClientCallbacks {
  onStatusChange?: (status: WsStatus) => void
  onPairingAck?: (payload: PairingAckPayload) => void
  onPairingReject?: (payload: PairingRejectPayload) => void
  onInquiryPush?: (payload: InquiryPushPayload) => void
  onModeToggleAck?: (payload: ModeToggleAckPayload) => void
}

/** Per-inquiry ack/error handler (IG4) */
interface AckHandler {
  onAck: (payload: InquiryAckPayload) => void
  onError: (payload: InquiryErrorPayload) => void
}

/**
 * IG5: mode-toggle ack handler registered via subscribeModeAck.
 * Receives both the ack and a boolean indicating success (applied).
 */
type ModeAckHandler = (payload: ModeToggleAckPayload) => void

/** Narrow raw payload to InquiryPushPayload with required field checks */
function narrowInquiryPushPayload(payload: unknown): InquiryPushPayload | null {
  if (typeof payload !== 'object' || payload === null) return null
  const p = payload as Record<string, unknown>
  if (
    typeof p.tool_use_id !== 'string' ||
    typeof p.session_id !== 'string' ||
    typeof p.tmux_session !== 'string' ||
    typeof p.header !== 'string' ||
    !Array.isArray(p.questions) ||
    typeof p.created_at !== 'string'
  ) {
    return null
  }
  // Validate questions array shape (at least check first item if present)
  const questions = p.questions as unknown[]
  for (const q of questions) {
    if (typeof q !== 'object' || q === null) return null
    const qi = q as Record<string, unknown>
    if (
      typeof qi.question !== 'string' ||
      !Array.isArray(qi.options) ||
      typeof qi.multiSelect !== 'boolean'
    ) {
      return null
    }
  }
  return payload as InquiryPushPayload
}

function narrowPairingAckPayload(payload: unknown): PairingAckPayload | null {
  if (typeof payload !== 'object' || payload === null) return null
  const p = payload as Record<string, unknown>
  if (typeof p.session !== 'string' || typeof p.server_version !== 'string') return null
  return payload as PairingAckPayload
}

function narrowPairingRejectPayload(payload: unknown): PairingRejectPayload | null {
  if (typeof payload !== 'object' || payload === null) return null
  const p = payload as Record<string, unknown>
  if (typeof p.reason !== 'string') return null
  return payload as PairingRejectPayload
}

function narrowInquiryAckPayload(payload: unknown): InquiryAckPayload | null {
  if (typeof payload !== 'object' || payload === null) return null
  const p = payload as Record<string, unknown>
  if (typeof p.tool_use_id !== 'string' || typeof p.latency_ms !== 'number') return null
  return payload as InquiryAckPayload
}

function narrowInquiryErrorPayload(payload: unknown): InquiryErrorPayload | null {
  if (typeof payload !== 'object' || payload === null) return null
  const p = payload as Record<string, unknown>
  if (typeof p.tool_use_id !== 'string' || typeof p.reason !== 'string') return null
  const validReasons = ['dialog-not-ready', 'send-keys-failed', 'inquiry-stale', 'validation']
  if (!validReasons.includes(p.reason as string)) return null
  return payload as InquiryErrorPayload
}

function narrowModeToggleAckPayload(payload: unknown): ModeToggleAckPayload | null {
  if (typeof payload !== 'object' || payload === null) return null
  const p = payload as Record<string, unknown>
  if (typeof p.mode !== 'string' || typeof p.applied !== 'boolean') return null
  return payload as ModeToggleAckPayload
}

export class WsClient {
  private _url: string | null = null
  private _token: string | null = null
  private _ws: WebSocket | null = null
  private _status: WsStatus = 'idle'
  private _callbacks: WsClientCallbacks = {}
  // IG4: per-tool_use_id ack handlers (replaces global callback swap)
  private _ackHandlers = new Map<string, AckHandler>()
  // IG5: single mode-ack subscriber (at most one PermissionModeView is mounted at a time)
  private _modeAckHandler: ModeAckHandler | null = null
  private _reconnect: ReconnectScheduler

  constructor() {
    this._reconnect = new ReconnectScheduler(
      () => this._attemptConnect(),
      () => this._handleTimeout(),
    )
  }

  setCallbacks(callbacks: WsClientCallbacks): void {
    this._callbacks = callbacks
  }

  getCallbacks(): WsClientCallbacks {
    return { ...this._callbacks }
  }

  /**
   * IG4: Subscribe to ack/error for a specific tool_use_id.
   * Returns an unsubscribe function — call it in useEffect cleanup.
   */
  subscribeAck(toolUseId: string, handler: AckHandler): () => void {
    this._ackHandlers.set(toolUseId, handler)
    return () => {
      this._ackHandlers.delete(toolUseId)
    }
  }

  /**
   * IG5: Subscribe to mode-toggle-ack messages.
   * At most one handler is active at a time (only one PermissionModeView mounts at once).
   * Returns an unsubscribe function — call it in useEffect cleanup to prevent stale handlers.
   *
   * Usage (in PermissionModeView):
   *   useEffect(() => wsClient.subscribeModeAck(handler), [handler])
   */
  subscribeModeAck(handler: ModeAckHandler): () => void {
    this._modeAckHandler = handler
    return () => {
      // Only clear if this specific handler is still registered (guard against double-cleanup)
      if (this._modeAckHandler === handler) {
        this._modeAckHandler = null
      }
    }
  }

  connect(wsUrl: string, token: string): void {
    // Build wss URL from tunnel URL
    // IG1: regex was /^https?:\/\// — correct for both http/https input
    const url = wsUrl.replace(/^https?:\/\//, 'wss://')
    this._url = url
    this._token = token
    this._setStatus('connecting')
    this._attemptConnect()
  }

  disconnect(): void {
    this._reconnect.reset()
    if (this._ws) {
      this._ws.close(1000, 'user-disconnect')
      this._ws = null
    }
    this._setStatus('idle')
  }

  send(envelope: Envelope): void {
    if (this._ws?.readyState === WebSocket.OPEN) {
      this._ws.send(JSON.stringify(envelope))
    }
  }

  /** Send a mode-toggle-request envelope */
  sendModeToggle(mode: 'plan' | 'accept-edits' | 'default'): void {
    this.send(makeEnvelope('mode-toggle-request', { mode }))
  }

  get status(): WsStatus {
    return this._status
  }

  getReconnectAttempt(): number {
    return this._reconnect.state.attempt
  }

  private _attemptConnect(): void {
    if (!this._url || !this._token) return

    this._setStatus(this._reconnect.state.attempt > 0 ? 'reconnecting' : 'connecting')

    try {
      const ws = new WebSocket(this._url)
      this._ws = ws

      ws.onopen = () => {
        // Send pairing-request as first frame (architecture.md §2.1)
        const pairingEnv = makeEnvelope('pairing-request', { token: this._token })
        ws.send(JSON.stringify(pairingEnv))
        // Note: status stays 'connecting' until pairing-ack received
      }

      ws.onmessage = (evt: MessageEvent<unknown>) => {
        this._handleMessage(evt.data)
      }

      ws.onclose = (evt: CloseEvent) => {
        // IG1: no heartbeat to clear

        if (evt.code === 4001) {
          // Token mismatch — go to pairing error, don't reconnect
          this._setStatus('pairing-error')
          return
        }
        if (evt.code === 4002) {
          // IG4: replaced by another client — distinct from auth failure (4001 / token-mismatch).
          // Use 'replaced' reason so PairingErrorView can show the correct "다른 기기에서 연결됨" UX.
          this._setStatus('pairing-error')
          this._callbacks.onPairingReject?.({
            reason: 'replaced',
          })
          return
        }
        if (evt.code === 4003) {
          // IG3: server shutdown — set 'disconnected' (was: 'disconnected' as WsStatus cast)
          this._setStatus('disconnected')
          return
        }

        // Unplanned close (1006, network drop, etc.) — schedule reconnect
        // IG3: call start() so visibilitychange listener + budget anchor are set up
        this._reconnect.start()
      }

      ws.onerror = () => {
        // onerror always followed by onclose; cleanup handled there
      }
    } catch {
      // WebSocket constructor can throw on invalid URL
      this._reconnect.scheduleNext()
    }
  }

  private _handleMessage(data: unknown): void {
    if (typeof data !== 'string') return

    let parsed: unknown
    try {
      parsed = JSON.parse(data)
    } catch {
      return
    }

    const env = parseEnvelope(parsed)
    if (!env) return

    switch (env.type) {
      case 'pairing-ack': {
        const payload = narrowPairingAckPayload(env.payload)
        if (!payload) return
        this._reconnect.reset()
        this._setStatus('connected')
        // IG1: no heartbeat sender — daemon sends Ping, browser auto-Pong
        this._callbacks.onPairingAck?.(payload)
        break
      }
      case 'pairing-reject': {
        const payload = narrowPairingRejectPayload(env.payload)
        if (!payload) return
        this._setStatus('pairing-error')
        this._callbacks.onPairingReject?.(payload)
        break
      }
      case 'inquiry-push': {
        // IG1: per-type narrowing for inquiry-push (UI directly depends on questions[0].options[])
        const payload = narrowInquiryPushPayload(env.payload)
        if (!payload) return
        this._callbacks.onInquiryPush?.(payload)
        break
      }
      case 'inquiry-ack': {
        const payload = narrowInquiryAckPayload(env.payload)
        if (!payload) return
        // IG4: route to per-inquiry handler first
        const handler = this._ackHandlers.get(payload.tool_use_id)
        if (handler) {
          handler.onAck(payload)
        }
        break
      }
      case 'inquiry-error': {
        const payload = narrowInquiryErrorPayload(env.payload)
        if (!payload) return
        // IG4: route to per-inquiry handler first
        const handler = this._ackHandlers.get(payload.tool_use_id)
        if (handler) {
          handler.onError(payload)
        }
        break
      }
      case 'mode-toggle-ack': {
        const payload = narrowModeToggleAckPayload(env.payload)
        if (!payload) return
        // IG5: route to the dedicated subscriber first (PermissionModeView via subscribeModeAck),
        // fall back to global onModeToggleAck callback (App.tsx mode-update bridge).
        if (this._modeAckHandler) {
          this._modeAckHandler(payload)
        } else {
          this._callbacks.onModeToggleAck?.(payload)
        }
        break
      }
      default:
        break
    }
  }

  private _handleTimeout(): void {
    this._setStatus('pairing-error')
  }

  private _setStatus(status: WsStatus): void {
    if (this._status !== status) {
      this._status = status
      this._callbacks.onStatusChange?.(status)
    }
  }
}

/** Singleton client instance */
export const wsClient = new WsClient()
