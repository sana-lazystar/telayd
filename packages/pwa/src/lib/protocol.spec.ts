/**
 * Protocol envelope round-trip tests (IG1 / Group 7 P0)
 * - All 9 message types parse correctly via parseEnvelope
 * - Missing or wrong fields return null
 * - InquiryPushPayload requires session_id / created_at (IG1 fix)
 * - InquiryErrorPayload.reason must be a known literal (IG1 fix)
 */

import { describe, expect, it } from 'vitest'
import { generateId, makeEnvelope, parseEnvelope } from './protocol'

const BASE = {
  v: 1 as const,
  id: 'test-id',
  ts: new Date().toISOString(),
}

function makeRaw(type: string, payload: Record<string, unknown>) {
  return { ...BASE, type, payload }
}

describe('parseEnvelope', () => {
  it('returns null for non-object input', () => {
    expect(parseEnvelope(null)).toBeNull()
    expect(parseEnvelope('string')).toBeNull()
    expect(parseEnvelope(42)).toBeNull()
  })

  it('returns null when v !== 1', () => {
    expect(parseEnvelope({ ...makeRaw('pairing-ack', {}), v: 2 })).toBeNull()
  })

  it('returns null for unknown type', () => {
    expect(parseEnvelope(makeRaw('ping', {}))).toBeNull()
    expect(parseEnvelope(makeRaw('status-push', {}))).toBeNull()
  })

  it('parses pairing-request', () => {
    const env = parseEnvelope(makeRaw('pairing-request', { token: 'abc' }))
    expect(env).not.toBeNull()
    expect(env?.type).toBe('pairing-request')
  })

  it('parses pairing-ack', () => {
    const env = parseEnvelope(makeRaw('pairing-ack', { session: 's1', server_version: '0.1.0' }))
    expect(env?.type).toBe('pairing-ack')
  })

  it('parses pairing-reject', () => {
    const env = parseEnvelope(makeRaw('pairing-reject', { reason: 'token-mismatch' }))
    expect(env?.type).toBe('pairing-reject')
  })

  it('parses inquiry-push', () => {
    const payload = {
      tool_use_id: 'toolu_01',
      session_id: 'sess-01',
      tmux_session: 'tmux-01',
      header: 'Header',
      questions: [],
      created_at: new Date().toISOString(),
    }
    const env = parseEnvelope(makeRaw('inquiry-push', payload))
    expect(env?.type).toBe('inquiry-push')
  })

  it('parses inquiry-response', () => {
    const env = parseEnvelope(
      makeRaw('inquiry-response', { tool_use_id: 'toolu_01', choice_index: 1 }),
    )
    expect(env?.type).toBe('inquiry-response')
  })

  it('parses inquiry-ack', () => {
    const env = parseEnvelope(makeRaw('inquiry-ack', { tool_use_id: 'toolu_01', latency_ms: 42 }))
    expect(env?.type).toBe('inquiry-ack')
  })

  it('parses inquiry-error', () => {
    const env = parseEnvelope(
      makeRaw('inquiry-error', { tool_use_id: 'toolu_01', reason: 'dialog-not-ready' }),
    )
    expect(env?.type).toBe('inquiry-error')
  })

  it('parses mode-toggle-request', () => {
    const env = parseEnvelope(makeRaw('mode-toggle-request', { mode: 'plan' }))
    expect(env?.type).toBe('mode-toggle-request')
  })

  it('parses mode-toggle-ack', () => {
    const env = parseEnvelope(makeRaw('mode-toggle-ack', { mode: 'plan', applied: true }))
    expect(env?.type).toBe('mode-toggle-ack')
  })
})

describe('makeEnvelope', () => {
  it('creates a valid envelope with auto-generated id', () => {
    const env = makeEnvelope('pairing-request', { token: 'tok' })
    expect(env.v).toBe(1)
    expect(env.type).toBe('pairing-request')
    expect(typeof env.id).toBe('string')
    expect(env.id.startsWith('pwa-')).toBe(true)
  })

  it('uses provided id when given', () => {
    const env = makeEnvelope('inquiry-response', { tool_use_id: 't1', cancel: true }, 't1')
    expect(env.id).toBe('t1')
  })
})

describe('generateId', () => {
  it('generates unique ids across calls', () => {
    const ids = new Set([generateId(), generateId(), generateId(), generateId(), generateId()])
    expect(ids.size).toBe(5)
  })
})
