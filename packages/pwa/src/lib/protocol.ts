/**
 * WebSocket envelope schema — matches daemon/src/protocol.rs
 * SSOT: architecture.md §2.3 (9 message types, L0 lock)
 */

export type MessageType =
  | 'pairing-request'
  | 'pairing-ack'
  | 'pairing-reject'
  | 'inquiry-push'
  | 'inquiry-response'
  | 'inquiry-ack'
  | 'inquiry-error'
  | 'mode-toggle-request'
  | 'mode-toggle-ack'

export interface Envelope<P = unknown> {
  v: 1
  type: MessageType
  id: string
  ts: string
  payload: P
}

// --- Payload types per message ---

export interface PairingRequestPayload {
  token: string
}

export interface PairingAckPayload {
  session: string
  server_version: string
}

export type PairingRejectReason =
  | 'token-mismatch'
  | 'expired'
  | 'bad-envelope'
  | 'bad-payload'
  | 'unsupported-version'

export interface PairingRejectPayload {
  reason: PairingRejectReason
}

export interface QuestionOption {
  index: number
  label: string
  description: string
}

export interface InquiryQuestion {
  question: string
  options: QuestionOption[]
  multiSelect: boolean
}

export interface InquiryPushPayload {
  tool_use_id: string
  session_id: string
  tmux_session: string
  header: string
  questions: InquiryQuestion[]
  created_at: string
  /** optional debug field (architecture.md §2.3 M-04) */
  permission_mode?: string
}

/** Exactly one of choice_index / free_text / cancel must be set */
export type InquiryResponsePayload =
  | { tool_use_id: string; choice_index: number; free_text?: undefined; cancel?: undefined }
  | { tool_use_id: string; choice_index?: undefined; free_text: string; cancel?: undefined }
  | { tool_use_id: string; choice_index?: undefined; free_text?: undefined; cancel: true }

export interface InquiryAckPayload {
  tool_use_id: string
  latency_ms: number
}

export type InquiryErrorReason =
  | 'dialog-not-ready'
  | 'send-keys-failed'
  | 'inquiry-stale'
  | 'validation'

export interface InquiryErrorPayload {
  tool_use_id: string
  reason: InquiryErrorReason
}

export type PermissionMode = 'plan' | 'accept-edits' | 'default'

export interface ModeToggleRequestPayload {
  mode: PermissionMode
}

export interface ModeToggleAckPayload {
  mode: PermissionMode
  applied: boolean
}

// --- WS close codes ---
export const WS_CLOSE = {
  NORMAL: 1000,
  PAIRING_MISMATCH: 4001,
  REPLACED: 4002,
  SERVER_SHUTDOWN: 4003,
} as const

// --- Helpers ---

let _seqCounter = 0

/** Generate a client-side correlation ID */
export function generateId(): string {
  _seqCounter = (_seqCounter + 1) % 1_000_000
  return `pwa-${Date.now()}-${_seqCounter}`
}

export function makeEnvelope<P>(type: MessageType, payload: P, id?: string): Envelope<P> {
  return {
    v: 1,
    type,
    id: id ?? generateId(),
    ts: new Date().toISOString(),
    payload,
  }
}

/** Type guard: narrow an unknown value to a typed Envelope */
export function parseEnvelope(raw: unknown): Envelope | null {
  if (typeof raw !== 'object' || raw === null) return null
  const obj = raw as Record<string, unknown>
  if (
    obj.v !== 1 ||
    typeof obj.type !== 'string' ||
    typeof obj.id !== 'string' ||
    typeof obj.ts !== 'string' ||
    typeof obj.payload !== 'object' ||
    obj.payload === null
  ) {
    return null
  }
  const validTypes: MessageType[] = [
    'pairing-request',
    'pairing-ack',
    'pairing-reject',
    'inquiry-push',
    'inquiry-response',
    'inquiry-ack',
    'inquiry-error',
    'mode-toggle-request',
    'mode-toggle-ack',
  ]
  if (!validTypes.includes(obj.type as MessageType)) return null
  return obj as unknown as Envelope
}
