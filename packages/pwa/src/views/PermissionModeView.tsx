/**
 * PermissionModeView (FE-pwa-5)
 * Anthropic #35637 정조준 — plan/accept-edits/default mode toggle
 * 5s ack timeout + retry 1회 + 사용자 알림
 *
 * Fix IG4: replaced prevCallbacks chain pattern with explicit cleanup ref.
 *   The previous pattern set callbacks inside handleApply but never restored them
 *   on rapid apply→cancel→apply cycles, accumulating stale handler chains.
 */

import { useEffect, useRef, useState } from 'react'
import { PermissionModeToggle } from '../components/PermissionModeToggle'
import { t } from '../lib/i18n'
import type { ModeToggleAckPayload, PermissionMode } from '../lib/protocol'
import { makeEnvelope } from '../lib/protocol'
import { wsClient } from '../lib/ws-client'
import styles from './PermissionModeView.module.css'

const ACK_TIMEOUT_MS = 5_000

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
  // IG4: store the unsubscribe fn so we can call it before each apply + on unmount
  const unsubscribeAckRef = useRef<(() => void) | null>(null)

  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
      // IG4: restore App-level onModeToggleAck on unmount
      unsubscribeAckRef.current?.()
    }
  }, [])

  function _registerAckHandler() {
    // IG4: Clean up any prior registration before registering a new one
    unsubscribeAckRef.current?.()

    const prevCallbacks = wsClient.getCallbacks()
    wsClient.setCallbacks({
      ...prevCallbacks,
      onModeToggleAck: (ackPayload: ModeToggleAckPayload) => {
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
        // IG4: restore previous handler after first ack
        wsClient.setCallbacks(prevCallbacks)
        unsubscribeAckRef.current = null
      },
    })

    // Store cleanup fn — restores prev callbacks if called before ack fires
    unsubscribeAckRef.current = () => {
      wsClient.setCallbacks(prevCallbacks)
      unsubscribeAckRef.current = null
    }
  }

  function sendToggle(mode: PermissionMode) {
    const env = makeEnvelope('mode-toggle-request', { mode })
    wsClient.send(env)

    if (timeoutRef.current) clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => {
      if (retryCountRef.current < 1) {
        retryCountRef.current++
        setApplyState('retrying')
        wsClient.send(env)
        timeoutRef.current = setTimeout(() => {
          setApplyState('error')
          setErrorMsg(t('permissionMode.timeout'))
          // IG4: restore on timeout (no ack will come)
          unsubscribeAckRef.current?.()
        }, ACK_TIMEOUT_MS)
      } else {
        setApplyState('error')
        setErrorMsg(t('permissionMode.timeout'))
        unsubscribeAckRef.current?.()
      }
    }, ACK_TIMEOUT_MS)
  }

  function handleApply() {
    if (applyState === 'applying' || applyState === 'retrying') return
    setApplyState('applying')
    retryCountRef.current = 0
    setErrorMsg('')

    // IG4: register ack handler with proper cleanup on each apply attempt
    _registerAckHandler()
    sendToggle(selected)
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
