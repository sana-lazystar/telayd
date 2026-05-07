/**
 * PermissionModeView (FE-pwa-5)
 * Anthropic #35637 정조준 — plan/accept-edits/default mode toggle
 * 5s ack timeout + retry 1회 + 사용자 알림
 *
 * Fix IG5: replaced getCallbacks/setCallbacks global-swap pattern with
 *   useEffect + wsClient.subscribeModeAck + cleanup-on-unmount.
 *   The swap pattern caused stale-handler chains on rapid apply→cancel→apply
 *   or any App.tsx useEffect re-fire. subscribeModeAck ensures exactly one
 *   handler is active at a time, with identity-guarded cleanup.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { PermissionModeToggle } from '../components/PermissionModeToggle'
import { t } from '../lib/i18n'
import type { ModeToggleAckPayload, PermissionMode } from '../lib/protocol'
import { wsClient } from '../lib/ws-client'
import styles from './PermissionModeView.module.css'

// IG5: shorten retry inner timeout from 5s to 3s (was: 5000+5000=10s total perceived freeze)
const ACK_TIMEOUT_MS = 5_000
const RETRY_TIMEOUT_MS = 3_000

interface Props {
  current: PermissionMode
  onApplied: (mode: PermissionMode) => void
  onCancel: () => void
}

type ApplyState = 'idle' | 'applying' | 'retrying' | 'applied' | 'error'

export function PermissionModeView({ current, onApplied, onCancel }: Props) {
  const [selected, setSelected] = useState<PermissionMode>(current)
  const [applyState, setApplyState] = useState<ApplyState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const retryCountRef = useRef(0)
  // IG5: track the pending mode so the ack handler can re-send on retry without closure capture
  const pendingModeRef = useRef<PermissionMode | null>(null)

  // Cleanup timers on unmount — subscribeModeAck cleanup is handled in its own useEffect
  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
    }
  }, [])

  // IG5: Register mode-ack subscriber via dedicated API (replaces getCallbacks/setCallbacks swap).
  // useCallback stabilises the handler identity so the useEffect dependency array is safe.
  const handleModeAck = useCallback(
    (ackPayload: ModeToggleAckPayload) => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
      if (ackPayload.applied) {
        setApplyState('applied')
        setTimeout(() => {
          onApplied(ackPayload.mode)
        }, 500)
      } else {
        setApplyState('error')
        setErrorMsg(t('permissionMode.notSupported'))
      }
    },
    [onApplied],
  )

  useEffect(() => {
    // Subscribe for the entire lifetime of this component.
    // subscribeModeAck returns the cleanup fn — React calls it on unmount.
    return wsClient.subscribeModeAck(handleModeAck)
  }, [handleModeAck])

  function _scheduleTimeout(mode: PermissionMode) {
    if (timeoutRef.current) clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => {
      if (retryCountRef.current < 1) {
        retryCountRef.current++
        setApplyState('retrying')
        // Re-send using the typed helper (IG5: consistent with subscribe API)
        wsClient.sendModeToggle(mode)
        // IG5: shorten inner retry timeout from 5s to 3s
        timeoutRef.current = setTimeout(() => {
          setApplyState('error')
          setErrorMsg(t('permissionMode.timeout'))
        }, RETRY_TIMEOUT_MS)
      } else {
        setApplyState('error')
        setErrorMsg(t('permissionMode.timeout'))
      }
    }, ACK_TIMEOUT_MS)
  }

  function handleApply() {
    if (applyState === 'applying' || applyState === 'retrying') return
    setApplyState('applying')
    retryCountRef.current = 0
    setErrorMsg('')
    pendingModeRef.current = selected

    // IG5: use typed helper instead of manual makeEnvelope (consistent with subscribe API)
    wsClient.sendModeToggle(selected)
    _scheduleTimeout(selected)
  }

  const isApplying = applyState === 'applying' || applyState === 'retrying'

  return (
    <main className={styles.container}>
      <header className={styles.header}>
        <h2 className={styles.title}>{t('permissionMode.title')}</h2>
        <p className={styles.subtitle}>{t('permissionMode.subtitle')}</p>
      </header>

      <div className={styles.content}>
        <PermissionModeToggle
          selected={selected}
          onChange={setSelected}
          disabled={isApplying || applyState === 'applied'}
        />
      </div>

      {applyState === 'error' && (
        <div className={styles.errorBanner} role="alert">
          <span>{errorMsg}</span>
        </div>
      )}

      {applyState === 'applied' && (
        <output className={styles.successBanner}>{t('permissionMode.applied')}</output>
      )}

      <footer className={styles.footer}>
        <button
          type="button"
          className={styles.cancelButton}
          onClick={onCancel}
          disabled={isApplying}
        >
          {t('permissionMode.cancelButton')}
        </button>
        <button
          type="button"
          className={styles.applyButton}
          onClick={handleApply}
          // IG2: removed `selected === current` from disabled predicate.
          // User must be able to re-confirm if there is a desync (daemon mode differs from local state).
          // Only `isApplying` and `applied` terminal state block the button.
          disabled={isApplying || applyState === 'applied'}
          aria-busy={isApplying}
        >
          {isApplying ? t('permissionMode.applying') : t('permissionMode.applyButton')}
        </button>
      </footer>
    </main>
  )
}
