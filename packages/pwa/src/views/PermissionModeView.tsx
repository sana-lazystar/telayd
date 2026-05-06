/**
 * PermissionModeView (FE-pwa-5)
 * Anthropic #35637 정조준 — plan/accept-edits/default mode toggle
 * 5s ack timeout + retry 1회 + 사용자 알림
 */

import { useEffect, useRef, useState } from 'react'
import { PermissionModeToggle } from '../components/PermissionModeToggle'
import type { PermissionMode } from '../lib/protocol'
import { makeEnvelope } from '../lib/protocol'
import { t } from '../lib/i18n'
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

  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
    }
  }, [])

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
        }, ACK_TIMEOUT_MS)
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

    // Register ack listener
    const prevCallbacks = wsClient.getCallbacks()
    wsClient.setCallbacks({
      ...prevCallbacks,
      onModeToggleAck: (ackPayload) => {
        if (timeoutRef.current) clearTimeout(timeoutRef.current)
        if (ackPayload.applied) {
          setApplyState('applied')
          setTimeout(() => {
            onApplied(ackPayload.mode)
          }, 500)
        } else {
          setApplyState('error')
          setErrorMsg(t('permissionMode.timeout'))
        }
        prevCallbacks.onModeToggleAck?.(ackPayload)
      },
    })

    sendToggle(selected)
  }

  const isApplying = applyState === 'applying' || applyState === 'retrying'

  return (
    <div className={styles.container} role="main">
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
        <div className={styles.successBanner} role="status">
          <span>{t('permissionMode.applied')}</span>
        </div>
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
          disabled={isApplying || selected === current || applyState === 'applied'}
          aria-busy={isApplying}
        >
          {isApplying ? t('permissionMode.applying') : t('permissionMode.applyButton')}
        </button>
      </footer>
    </div>
  )
}
