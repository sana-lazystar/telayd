/**
 * Exponential backoff reconnect scheduler
 * architecture.md §2.5: 0.5s → 2s → 4s → 8s, max 10s
 * visibilitychange foreground 복귀 시 즉시 attempt
 *
 * Fix IG3: budget anchored to Date.now() (was: _totalElapsedMs += delay pre-fire)
 * Fix IG3: first backoff reduced 1000→500ms to keep K8 < 10s on 4G (4 fails = ~15s → ~14.5s)
 */

// IG3: 500ms first step (was 1000) so 4 rapid fails stay under 10s total
const BACKOFF_STEPS_MS = [500, 2000, 4000, 8000, 10000]
const MAX_BACKOFF_MS = 10_000
const TOTAL_FAILURE_BUDGET_MS = 30_000

export type ReconnectState = {
  attempt: number
  totalElapsedMs: number
  nextDelayMs: number
  timedOut: boolean
}

export class ReconnectScheduler {
  private _attempt = 0
  // IG3: wall-clock budget tracking (was: pre-fire _totalElapsedMs += delay)
  private _startedAt: number | null = null
  private _timer: ReturnType<typeof setTimeout> | null = null
  private _onReconnect: () => void
  private _onTimeout: () => void
  private _visibilityHandler: (() => void) | null = null

  constructor(onReconnect: () => void, onTimeout: () => void) {
    this._onReconnect = onReconnect
    this._onTimeout = onTimeout
    // IG3: attach visibilitychange in constructor so it's always active
    this._attachVisibility()
  }

  /** Call when the connection drops to schedule first retry */
  start(): void {
    if (this._startedAt === null) {
      this._startedAt = Date.now()
    }
    this._schedule()
  }

  /** Call when connection succeeds to cancel any pending timers */
  reset(): void {
    this._attempt = 0
    this._startedAt = null
    this._clearTimer()
    this._detachVisibility()
    // Re-attach so the next connection can use foreground recovery
    this._attachVisibility()
  }

  /** Force immediate reconnect attempt (e.g. on visibilitychange) */
  immediate(): void {
    this._clearTimer()
    this._attempt = Math.max(0, this._attempt - 1) // don't penalise foreground return
    this._onReconnect()
  }

  get state(): ReconnectState {
    const totalElapsedMs = this._startedAt !== null ? Date.now() - this._startedAt : 0
    return {
      attempt: this._attempt,
      totalElapsedMs,
      nextDelayMs: this._nextDelay(),
      timedOut: totalElapsedMs >= TOTAL_FAILURE_BUDGET_MS,
    }
  }

  private _nextDelay(): number {
    const step = Math.min(this._attempt, BACKOFF_STEPS_MS.length - 1)
    return BACKOFF_STEPS_MS[step] ?? MAX_BACKOFF_MS
  }

  private _schedule(): void {
    // IG3: check elapsed AFTER timer fires, not before — use wall-clock
    const delay = this._nextDelay()

    this._timer = setTimeout(() => {
      this._attempt++
      // Check budget based on real elapsed time (not accumulated scheduled delays)
      const elapsed = this._startedAt !== null ? Date.now() - this._startedAt : 0
      if (elapsed > TOTAL_FAILURE_BUDGET_MS) {
        this._onTimeout()
        return
      }
      this._onReconnect()
    }, delay)
  }

  /** Called by ws-client after a failed attempt to reschedule */
  scheduleNext(): void {
    if (this._startedAt === null) {
      this._startedAt = Date.now()
    }
    this._schedule()
  }

  private _clearTimer(): void {
    if (this._timer !== null) {
      clearTimeout(this._timer)
      this._timer = null
    }
  }

  private _attachVisibility(): void {
    this._visibilityHandler = () => {
      if (document.visibilityState === 'visible') {
        this.immediate()
      }
    }
    document.addEventListener('visibilitychange', this._visibilityHandler)
  }

  private _detachVisibility(): void {
    if (this._visibilityHandler) {
      document.removeEventListener('visibilitychange', this._visibilityHandler)
      this._visibilityHandler = null
    }
  }
}
