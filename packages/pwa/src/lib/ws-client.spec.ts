/**
 * WsClient unit tests (IG2 / IG3 / IG4 / Group 7 P0)
 * - Close codes 4001/4002/4003/1000/1006 → correct WsStatus
 * - 4002 does not schedule reconnect
 * - subscribeAck per-tool_use_id routing
 * - No heartbeat sender (IG1)
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeEnvelope } from './protocol'
import { WsClient } from './ws-client'

// Minimal WebSocket mock
class MockWebSocket {
  static OPEN = 1
  static CLOSED = 3
  readyState = MockWebSocket.OPEN
  onopen: (() => void) | null = null
  onmessage: ((evt: { data: unknown }) => void) | null = null
  onclose: ((evt: { code: number; reason: string }) => void) | null = null
  onerror: (() => void) | null = null
  sentMessages: string[] = []

  constructor(public url: string) {}
  send(data: string) {
    this.sentMessages.push(data)
  }
  close(code?: number, reason?: string) {
    this.readyState = MockWebSocket.CLOSED
    this.onclose?.({ code: code ?? 1000, reason: reason ?? '' })
  }
  // Test helper: simulate server sending a message
  receive(data: string) {
    this.onmessage?.({ data })
  }
  // Test helper: simulate server initiating close
  serverClose(code: number, reason = '') {
    this.readyState = MockWebSocket.CLOSED
    this.onclose?.({ code, reason })
  }
  open() {
    this.onopen?.()
  }
}

let mockWs: MockWebSocket | null = null

describe('WsClient', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    // Patch global WebSocket
    vi.stubGlobal(
      'WebSocket',
      class extends MockWebSocket {
        constructor(url: string) {
          super(url)
          mockWs = this
        }
      },
    )
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
    mockWs = null
  })

  function makeClient() {
    const client = new WsClient()
    return client
  }

  function connectAndPair(client: WsClient) {
    client.connect('https://x.trycloudflare.com', 'x'.repeat(43))
    mockWs?.open()
    // Simulate pairing-ack
    const ack = makeEnvelope('pairing-ack', { session: 's1', server_version: '0.1.0' })
    mockWs?.receive(JSON.stringify(ack))
  }

  it('status is "connecting" after connect()', () => {
    const client = makeClient()
    client.connect('https://x.trycloudflare.com', 'x'.repeat(43))
    expect(client.status).toBe('connecting')
  })

  it('status is "connected" after pairing-ack', () => {
    const client = makeClient()
    connectAndPair(client)
    expect(client.status).toBe('connected')
  })

  it('close 4001 → pairing-error, no reconnect', () => {
    const onStatusChange = vi.fn()
    const client = makeClient()
    client.setCallbacks({ onStatusChange })
    connectAndPair(client)

    mockWs?.serverClose(4001)
    expect(client.status).toBe('pairing-error')
    // Ensure no reconnect timer fires
    vi.advanceTimersByTime(10_000)
    expect(onStatusChange).not.toHaveBeenCalledWith('reconnecting')
  })

  // IG2: 4002 should set pairing-error, NOT reconnect
  it('close 4002 → pairing-error, no reconnect (IG2)', () => {
    const onStatusChange = vi.fn()
    const client = makeClient()
    client.setCallbacks({ onStatusChange })
    connectAndPair(client)

    mockWs?.serverClose(4002)
    expect(client.status).toBe('pairing-error')
    vi.advanceTimersByTime(10_000)
    expect(onStatusChange).not.toHaveBeenCalledWith('reconnecting')
  })

  // IG3: 4003 should set 'disconnected', not cast as WsStatus
  it('close 4003 → disconnected (IG3)', () => {
    const onStatusChange = vi.fn()
    const client = makeClient()
    client.setCallbacks({ onStatusChange })
    connectAndPair(client)

    mockWs?.serverClose(4003)
    expect(client.status).toBe('disconnected')
    expect(onStatusChange).toHaveBeenCalledWith('disconnected')
  })

  // IG3: unplanned close (1006) schedules reconnect
  it('close 1006 → starts reconnect', () => {
    const onStatusChange = vi.fn()
    const client = makeClient()
    client.setCallbacks({ onStatusChange })
    connectAndPair(client)

    mockWs?.serverClose(1006)
    // After timer fires, status should be 'reconnecting'
    vi.advanceTimersByTime(600)
    expect(client.status).toBe('reconnecting')
  })

  // IG4: subscribeAck routes to correct handler by tool_use_id
  it('subscribeAck routes inquiry-ack to correct handler (IG4)', () => {
    const client = makeClient()
    connectAndPair(client)

    const onAck1 = vi.fn()
    const onAck2 = vi.fn()

    client.subscribeAck('toolu_01', { onAck: onAck1, onError: vi.fn() })
    client.subscribeAck('toolu_02', { onAck: onAck2, onError: vi.fn() })

    const ack = makeEnvelope('inquiry-ack', { tool_use_id: 'toolu_01', latency_ms: 42 })
    mockWs?.receive(JSON.stringify(ack))

    expect(onAck1).toHaveBeenCalledTimes(1)
    expect(onAck2).not.toHaveBeenCalled()
  })

  // IG4: subscribeAck returns unsubscribe function
  it('unsubscribe fn removes handler (IG4)', () => {
    const client = makeClient()
    connectAndPair(client)

    const onAck = vi.fn()
    const unsubscribe = client.subscribeAck('toolu_03', { onAck, onError: vi.fn() })
    unsubscribe()

    const ack = makeEnvelope('inquiry-ack', { tool_use_id: 'toolu_03', latency_ms: 10 })
    mockWs?.receive(JSON.stringify(ack))

    expect(onAck).not.toHaveBeenCalled()
  })

  // IG1: no heartbeat — verify no periodic messages sent after 60s
  it('no heartbeat ping sent after pairing-ack (IG1)', () => {
    const client = makeClient()
    connectAndPair(client)

    const msgCountAfterPairing = mockWs?.sentMessages.length ?? 0
    vi.advanceTimersByTime(60_000)
    const msgCountAfter60s = mockWs?.sentMessages.length ?? 0

    // Only the initial pairing-request should have been sent; no ping frames
    expect(msgCountAfter60s).toBe(msgCountAfterPairing)
  })

  // IG1: per-type narrowing — invalid inquiry-push payload is silently dropped
  it('inquiry-push with missing session_id is silently dropped (IG1)', () => {
    const client = makeClient()
    connectAndPair(client)
    const onInquiryPush = vi.fn()
    client.setCallbacks({ onInquiryPush })

    // Missing session_id and created_at
    const bad = makeEnvelope('inquiry-push', {
      tool_use_id: 'toolu_01',
      tmux_session: 'tmux-01',
      header: 'H',
      questions: [],
    })
    mockWs?.receive(JSON.stringify(bad))
    expect(onInquiryPush).not.toHaveBeenCalled()
  })
})
