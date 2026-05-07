/**
 * App state machine (FE-pwa-4)
 * 7 page IDs: pairing / connecting / idle / prompt-choice / permission-mode / disconnected / pairing-error
 * useReducer based — no external store (architecture.md §7 ADR)
 */

import type { InquiryPushPayload, PermissionMode } from '../lib/protocol'

export type PageId =
  | 'pairing'
  | 'connecting'
  | 'idle'
  | 'prompt-choice'
  | 'permission-mode'
  | 'disconnected'
  | 'pairing-error'

export interface AppState {
  page: PageId
  /** Current tunnel URL (set after successful pairing) */
  tunnelUrl: string
  /** Current mode (updated after mode-toggle-ack) */
  permissionMode: PermissionMode
  /** Active inquiry pushed from daemon */
  activeInquiry: InquiryPushPayload | null
  /** WS reconnect attempt count (for display) */
  reconnectAttempt: number
  /** Pairing error reason */
  pairingErrorReason: string | null
  /** Server session id */
  sessionId: string | null
  /** Server version */
  serverVersion: string | null
  /**
   * IG2: set to true when INQUIRY_PUSH arrives while permission-mode toggle was in-flight.
   * PromptChoiceView reads this flag to surface a transient toast, then clears it.
   */
  pendingModeAbandoned: boolean
}

const initialState: AppState = {
  page: 'pairing',
  tunnelUrl: '',
  permissionMode: 'default',
  activeInquiry: null,
  reconnectAttempt: 0,
  pairingErrorReason: null,
  sessionId: null,
  serverVersion: null,
  pendingModeAbandoned: false,
}

export type AppAction =
  | { type: 'CONNECT_START'; tunnelUrl: string }
  | { type: 'PAIRING_ACK'; sessionId: string; serverVersion: string }
  | { type: 'PAIRING_REJECT'; reason: string }
  | { type: 'INQUIRY_PUSH'; payload: InquiryPushPayload }
  | { type: 'INQUIRY_RESOLVED' }
  | { type: 'OPEN_PERMISSION_MODE' }
  | { type: 'CLOSE_PERMISSION_MODE' }
  | { type: 'MODE_UPDATED'; mode: PermissionMode }
  | { type: 'WS_DISCONNECTED' }
  | { type: 'WS_RECONNECTING'; attempt: number }
  | { type: 'WS_TIMEOUT' }
  | { type: 'RESET_TO_PAIRING' }
  /** IG2: dismiss the pendingModeAbandoned toast after it has been shown */
  | { type: 'CLEAR_MODE_ABANDONED' }

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'CONNECT_START':
      return { ...state, page: 'connecting', tunnelUrl: action.tunnelUrl }

    case 'PAIRING_ACK':
      return {
        ...state,
        page: 'idle',
        sessionId: action.sessionId,
        serverVersion: action.serverVersion,
        reconnectAttempt: 0,
        pairingErrorReason: null,
      }

    case 'PAIRING_REJECT':
      return {
        ...state,
        page: 'pairing-error',
        pairingErrorReason: action.reason,
      }

    case 'INQUIRY_PUSH':
      // IG2: if a mode toggle was in-flight (page was 'permission-mode'), surface a toast
      // so the user knows their in-progress toggle was abandoned by the arriving prompt.
      return {
        ...state,
        page: 'prompt-choice',
        activeInquiry: action.payload,
        pendingModeAbandoned: state.page === 'permission-mode',
      }

    case 'INQUIRY_RESOLVED':
      return {
        ...state,
        page: 'idle',
        activeInquiry: null,
      }

    case 'OPEN_PERMISSION_MODE':
      return { ...state, page: 'permission-mode' }

    case 'CLOSE_PERMISSION_MODE':
      return { ...state, page: 'idle' }

    case 'MODE_UPDATED':
      return { ...state, permissionMode: action.mode, page: 'idle' }

    case 'WS_DISCONNECTED':
      return {
        ...state,
        page: 'disconnected',
        activeInquiry: null,
      }

    case 'WS_RECONNECTING':
      return {
        ...state,
        page: 'disconnected',
        reconnectAttempt: action.attempt,
      }

    case 'WS_TIMEOUT':
      return {
        ...state,
        page: 'pairing-error',
        pairingErrorReason: 'timeout',
      }

    case 'RESET_TO_PAIRING':
      return {
        ...initialState,
        // preserve last URL for convenience
        tunnelUrl: state.tunnelUrl,
      }

    // IG2: clear the deferred-apply toast once PromptChoiceView has shown it
    case 'CLEAR_MODE_ABANDONED':
      return { ...state, pendingModeAbandoned: false }

    default:
      return state
  }
}

export { initialState }
