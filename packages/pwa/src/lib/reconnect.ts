/**
 * Exponential backoff reconnect scheduler
 * architecture.md §2.5: 1s → 2s → 4s → 8s, max 10s
 * visibilitychange foreground 복귀 시 즉시 attempt
 */

const BACKOFF_STEPS_MS = [1000, 2000, 4000, 8000, 10000]
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
  private _totalElapsedMs = 0
  private _timer: ReturnType<typeof setTimeout> | null = null
  private _onReconnect: () => void
  private _onTimeout: () => void
  private _visibilityHandler: (() => void) | null = null

  constructor(onReconnect: () => void, onTimeout: () => void) {
    this._onReconnect = onReconnect
    this._onTimeout = onTimeout
  }

  /** Call when the connection drops to schedule first retry */
  start(): void {
    this._schedule()
    this._attachVisibility()
  }

  /** Call when connection succeeds to cancel any pending timers */
  reset(): void {
    this._attempt = 0
    this._totalElapsedMs = 0
    this._clearTimer()
    this._detachVisibility()
  }

  /** Force immediate reconnect attempt (e.g. on visibilitychange) */
  immediate(): void {
    this._clearTimer()
    this._attempt = Math.max(0, this._attempt - 1) // don't penalise foreground return
    this._onReconnect()
  }

  get state(): ReconnectState {
    return {
      attempt: this._attempt,
      totalElapsedMs: this._totalElapsedMs,
      nextDelayMs: this._nextDelay(),
      timedOut: this._totalElapsedMs >= TOTAL_FAILURE_BUDGET_MS,
    }
  }

  private _nextDelay(): number {
    const step = Math.min(this._attempt, BACKOFF_STEPS_MS.length - 1)
    return BACKOFF_STEPS_MS[step] ?? MAX_BACKOFF_MS
  }

  private _schedule(): void {
    const delay = this._nextDelay()
    this._totalElapsedMs += delay

    if (this._totalElapsedMs > TOTAL_FAILURE_BUDGET_MS) {
      this._onTimeout()
      return
    }

    this._timer = setTimeout(() => {
      this._attempt++
      this._onReconnect()
    }, delay)
  }

  /** Called by ws-client after a failed attempt to reschedule */
  scheduleNext(): void {
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
