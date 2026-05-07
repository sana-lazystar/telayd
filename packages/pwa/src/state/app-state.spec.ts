/**
 * App state reducer tests (IG3 / Group 7 P0)
 * - All 7 page transitions including WS_DISCONNECTED (was dead branch)
 * - IG3: WS_DISCONNECTED dispatch coverage
 */

import { describe, expect, it } from 'vitest'
import { appReducer, initialState } from './app-state'
import type { AppAction, AppState } from './app-state'

const MOCK_INQUIRY = {
  tool_use_id: 'toolu_01',
  session_id: 'sess-01',
  tmux_session: 'tmux-01',
  header: 'Test',
  questions: [],
  created_at: new Date().toISOString(),
}

describe('appReducer', () => {
  it('initial state is pairing page', () => {
    expect(initialState.page).toBe('pairing')
  })

  it('CONNECT_START → connecting page', () => {
    const state = appReducer(initialState, { type: 'CONNECT_START', tunnelUrl: 'wss://x.y.z' })
    expect(state.page).toBe('connecting')
    expect(state.tunnelUrl).toBe('wss://x.y.z')
  })

  it('PAIRING_ACK → idle page', () => {
    const connecting: AppState = { ...initialState, page: 'connecting' }
    const state = appReducer(connecting, {
      type: 'PAIRING_ACK',
      sessionId: 's1',
      serverVersion: '0.1.0',
    })
    expect(state.page).toBe('idle')
    expect(state.sessionId).toBe('s1')
    expect(state.reconnectAttempt).toBe(0)
  })

  it('PAIRING_REJECT → pairing-error page', () => {
    const state = appReducer(initialState, { type: 'PAIRING_REJECT', reason: 'token-mismatch' })
    expect(state.page).toBe('pairing-error')
    expect(state.pairingErrorReason).toBe('token-mismatch')
  })

  it('INQUIRY_PUSH → prompt-choice page', () => {
    const idle: AppState = { ...initialState, page: 'idle' }
    const state = appReducer(idle, { type: 'INQUIRY_PUSH', payload: MOCK_INQUIRY })
    expect(state.page).toBe('prompt-choice')
    expect(state.activeInquiry).toEqual(MOCK_INQUIRY)
  })

  it('INQUIRY_RESOLVED → idle page (clears activeInquiry)', () => {
    const withInquiry: AppState = {
      ...initialState,
      page: 'prompt-choice',
      activeInquiry: MOCK_INQUIRY,
    }
    const state = appReducer(withInquiry, { type: 'INQUIRY_RESOLVED' })
    expect(state.page).toBe('idle')
    expect(state.activeInquiry).toBeNull()
  })

  it('OPEN_PERMISSION_MODE → permission-mode page', () => {
    const idle: AppState = { ...initialState, page: 'idle' }
    const state = appReducer(idle, { type: 'OPEN_PERMISSION_MODE' })
    expect(state.page).toBe('permission-mode')
  })

  it('CLOSE_PERMISSION_MODE → idle page', () => {
    const pm: AppState = { ...initialState, page: 'permission-mode' }
    const state = appReducer(pm, { type: 'CLOSE_PERMISSION_MODE' })
    expect(state.page).toBe('idle')
  })

  it('MODE_UPDATED → idle page with new mode', () => {
    const state = appReducer(initialState, { type: 'MODE_UPDATED', mode: 'plan' })
    expect(state.page).toBe('idle')
    expect(state.permissionMode).toBe('plan')
  })

  // IG3: WS_DISCONNECTED was defined in reducer but never dispatched
  it('WS_DISCONNECTED → disconnected page (clears activeInquiry)', () => {
    const connected: AppState = { ...initialState, page: 'idle', activeInquiry: MOCK_INQUIRY }
    const state = appReducer(connected, { type: 'WS_DISCONNECTED' })
    expect(state.page).toBe('disconnected')
    expect(state.activeInquiry).toBeNull()
  })

  it('WS_RECONNECTING → disconnected page with attempt count', () => {
    const state = appReducer(initialState, { type: 'WS_RECONNECTING', attempt: 3 })
    expect(state.page).toBe('disconnected')
    expect(state.reconnectAttempt).toBe(3)
  })

  it('WS_TIMEOUT → pairing-error page', () => {
    const state = appReducer(initialState, { type: 'WS_TIMEOUT' })
    expect(state.page).toBe('pairing-error')
    expect(state.pairingErrorReason).toBe('timeout')
  })

  it('RESET_TO_PAIRING → initial state (preserves tunnelUrl)', () => {
    const withUrl: AppState = { ...initialState, page: 'idle', tunnelUrl: 'wss://kept.com' }
    const state = appReducer(withUrl, { type: 'RESET_TO_PAIRING' })
    expect(state.page).toBe('pairing')
    expect(state.tunnelUrl).toBe('wss://kept.com')
  })

  it('unknown action returns state unchanged', () => {
    // Type assertion to test the default case
    const state = appReducer(initialState, { type: 'UNKNOWN_ACTION' } as unknown as AppAction)
    expect(state).toBe(initialState)
  })

  // IG2: INQUIRY_PUSH from permission-mode page sets pendingModeAbandoned
  it('INQUIRY_PUSH from permission-mode page → sets pendingModeAbandoned (IG2)', () => {
    const inModeToggle: AppState = { ...initialState, page: 'permission-mode' }
    const state = appReducer(inModeToggle, { type: 'INQUIRY_PUSH', payload: MOCK_INQUIRY })
    expect(state.page).toBe('prompt-choice')
    expect(state.pendingModeAbandoned).toBe(true)
  })

  // IG2: INQUIRY_PUSH from other pages does NOT set pendingModeAbandoned
  it('INQUIRY_PUSH from idle page → pendingModeAbandoned stays false (IG2)', () => {
    const idle: AppState = { ...initialState, page: 'idle' }
    const state = appReducer(idle, { type: 'INQUIRY_PUSH', payload: MOCK_INQUIRY })
    expect(state.pendingModeAbandoned).toBe(false)
  })

  // IG2: CLEAR_MODE_ABANDONED clears the flag
  it('CLEAR_MODE_ABANDONED → pendingModeAbandoned set to false (IG2)', () => {
    const withFlag: AppState = { ...initialState, pendingModeAbandoned: true }
    const state = appReducer(withFlag, { type: 'CLEAR_MODE_ABANDONED' })
    expect(state.pendingModeAbandoned).toBe(false)
  })
})
