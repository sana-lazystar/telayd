/**
 * WebSocket Client (FE-pwa-3)
 * - native WebSocket API
 * - exponential backoff reconnect (architecture.md §2.5)
 * - visibilitychange foreground hook
 * - envelope serialize/deserialize (protocol.ts)
 * Security: token never logged, envelope schema validated
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

export type WsStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'pairing-error'

export interface WsClientCallbacks {
  onStatusChange?: (status: WsStatus) => void
  onPairingAck?: (payload: PairingAckPayload) => void
  onPairingReject?: (payload: PairingRejectPayload) => void
  onInquiryPush?: (payload: InquiryPushPayload) => void
  onInquiryAck?: (payload: InquiryAckPayload) => void
  onInquiryError?: (payload: InquiryErrorPayload) => void
  onModeToggleAck?: (payload: ModeToggleAckPayload) => void
}

export class WsClient {
  private _url: string | null = null
  private _token: string | null = null
  private _ws: WebSocket | null = null
  private _status: WsStatus = 'idle'
  private _callbacks: WsClientCallbacks = {}
  private _heartbeatTimer: ReturnType<typeof setInterval> | null = null
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

  connect(wsUrl: string, token: string): void {
    // Build wss URL from tunnel URL
    const url = wsUrl.replace(/^https?:\/\//, 'wss://')
    this._url = url
    this._token = token
    this._setStatus('connecting')
    this._attemptConnect()
  }

  disconnect(): void {
    this._reconnect.reset()
    this._clearHeartbeat()
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
        this._clearHeartbeat()

        if (evt.code === 4001) {
          // Token mismatch — go to pairing error, don't reconnect
          this._setStatus('pairing-error')
          return
        }
        if (evt.code === 4003) {
          // Server shutdown — don't auto-reconnect
          this._setStatus('disconnected' as WsStatus)
          return
        }

        // Schedule reconnect
        this._reconnect.scheduleNext()
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
        this._reconnect.reset()
        this._setStatus('connected')
        this._startHeartbeat()
        this._callbacks.onPairingAck?.(env.payload as PairingAckPayload)
        break
      }
      case 'pairing-reject': {
        this._setStatus('pairing-error')
        this._callbacks.onPairingReject?.(env.payload as PairingRejectPayload)
        break
      }
      case 'inquiry-push': {
        this._callbacks.onInquiryPush?.(env.payload as InquiryPushPayload)
        break
      }
      case 'inquiry-ack': {
        this._callbacks.onInquiryAck?.(env.payload as InquiryAckPayload)
        break
      }
      case 'inquiry-error': {
        this._callbacks.onInquiryError?.(env.payload as InquiryErrorPayload)
        break
      }
      case 'mode-toggle-ack': {
        this._callbacks.onModeToggleAck?.(env.payload as ModeToggleAckPayload)
        break
      }
      default:
        break
    }
  }

  private _startHeartbeat(): void {
    // Send ping every 30s (architecture.md §2.1)
    this._clearHeartbeat()
    this._heartbeatTimer = setInterval(() => {
      if (this._ws?.readyState === WebSocket.OPEN) {
        this._ws.send(JSON.stringify({ v: 1, type: 'ping', id: 'hb', ts: new Date().toISOString(), payload: {} }))
      }
    }, 30_000)
  }

  private _clearHeartbeat(): void {
    if (this._heartbeatTimer !== null) {
      clearInterval(this._heartbeatTimer)
      this._heartbeatTimer = null
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
