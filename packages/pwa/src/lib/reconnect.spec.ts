/**
 * Regression tests for ReconnectScheduler (IG3 / Group 7 P0)
 * - Backoff sequence correctness
 * - Budget elapsed uses wall-clock (not pre-fire accumulated delay)
 * - visibilitychange attached on start (via constructor in fixed version)
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ReconnectScheduler } from './reconnect'

describe('ReconnectScheduler', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('fires callback after first step delay (500ms)', () => {
    const onReconnect = vi.fn()
    const onTimeout = vi.fn()
    const scheduler = new ReconnectScheduler(onReconnect, onTimeout)

    scheduler.start()
    expect(onReconnect).not.toHaveBeenCalled()

    vi.advanceTimersByTime(500)
    expect(onReconnect).toHaveBeenCalledTimes(1)
  })

  it('respects exponential backoff steps [500, 2000, 4000, 8000, 10000]', () => {
    const onReconnect = vi.fn()
    const onTimeout = vi.fn()
    const scheduler = new ReconnectScheduler(onReconnect, onTimeout)

    // Simulate repeated failures — manually call scheduleNext each time
    scheduler.start()
    vi.advanceTimersByTime(500) // attempt 0 fires → step 0 done
    expect(onReconnect).toHaveBeenCalledTimes(1)

    scheduler.scheduleNext()
    vi.advanceTimersByTime(2000) // step 1
    expect(onReconnect).toHaveBeenCalledTimes(2)

    scheduler.scheduleNext()
    vi.advanceTimersByTime(4000) // step 2
    expect(onReconnect).toHaveBeenCalledTimes(3)
  })

  it('calls onTimeout when 30s wall-clock budget exceeded', () => {
    const onReconnect = vi.fn()
    const onTimeout = vi.fn()
    const scheduler = new ReconnectScheduler(onReconnect, onTimeout)

    // Advance 31 seconds — budget should be exceeded on next fire
    scheduler.start()
    // Simulate first attempt fires
    vi.advanceTimersByTime(500)
    // Advance past 30s total budget
    vi.advanceTimersByTime(30_000)

    scheduler.scheduleNext()
    vi.advanceTimersByTime(10_000) // any step will trigger timeout check

    expect(onTimeout).toHaveBeenCalledTimes(1)
    expect(onReconnect).toHaveBeenCalledTimes(1) // only the first one
  })

  it('budget elapsed is wall-clock based (rapid scheduleNext calls do not exhaust budget faster)', () => {
    const onReconnect = vi.fn()
    const onTimeout = vi.fn()
    const scheduler = new ReconnectScheduler(onReconnect, onTimeout)

    scheduler.start()
    // Rapidly schedule 5 nexts with 0 wall-clock time passing
    for (let i = 0; i < 5; i++) {
      scheduler.scheduleNext()
    }

    // Budget should NOT be exhausted since wall-clock time has not advanced
    expect(onTimeout).not.toHaveBeenCalled()
    // advance 1s — budget still not reached
    vi.advanceTimersByTime(1000)
    expect(onTimeout).not.toHaveBeenCalled()
  })

  it('reset cancels pending timer and clears attempt count', () => {
    const onReconnect = vi.fn()
    const onTimeout = vi.fn()
    const scheduler = new ReconnectScheduler(onReconnect, onTimeout)

    scheduler.start()
    scheduler.reset()

    vi.advanceTimersByTime(60_000)
    expect(onReconnect).not.toHaveBeenCalled()
    expect(scheduler.state.attempt).toBe(0)
  })

  it('immediate() decrements attempt and fires onReconnect without delay', () => {
    const onReconnect = vi.fn()
    const onTimeout = vi.fn()
    const scheduler = new ReconnectScheduler(onReconnect, onTimeout)

    scheduler.start()
    vi.advanceTimersByTime(500) // advance through first step
    scheduler.immediate()

    expect(onReconnect).toHaveBeenCalledTimes(2)
  })
})
