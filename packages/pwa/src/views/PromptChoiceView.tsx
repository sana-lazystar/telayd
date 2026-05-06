/**
 * PromptChoiceView (FE-pwa-2)
 * Anthropic #29214/#29438 정조준
 * 4-button choice UI + free-text textarea + mode badge
 * 5s timeout retry 1회 (B4 mitigation)
 */

import { useEffect, useRef, useState } from 'react'
import { ChoiceButton } from '../components/ChoiceButton'
import { PromptContextBlock } from '../components/PromptContextBlock'
import { t } from '../lib/i18n'
import type { InquiryPushPayload, InquiryResponsePayload, PermissionMode } from '../lib/protocol'
import { makeEnvelope } from '../lib/protocol'
import { wsClient } from '../lib/ws-client'
import styles from './PromptChoiceView.module.css'

const RESPONSE_TIMEOUT_MS = 5_000
const MAX_FREE_TEXT_LENGTH = 4096

interface Props {
  inquiry: InquiryPushPayload
  permissionMode: PermissionMode
  onResolved: () => void
  onOpenModeToggle: () => void
}

type SendState = 'idle' | 'sending' | 'retrying' | 'error'

export function PromptChoiceView({ inquiry, permissionMode, onResolved, onOpenModeToggle }: Props) {
  const question = inquiry.questions[0]
  const isMultiSelect = question?.multiSelect ?? false
  const options = question?.options ?? []

  const [sendState, setSendState] = useState<SendState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const [freeTextExpanded, setFreeTextExpanded] = useState(false)
  const [freeText, setFreeText] = useState('')
  const [activeIndex, setActiveIndex] = useState<number | null>(null)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const retryCountRef = useRef(0)

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
    }
  }, [])

  function sendResponse(payload: InquiryResponsePayload) {
    const env = makeEnvelope('inquiry-response', payload, payload.tool_use_id)
    wsClient.send(env)

    // Set timeout for ack
    if (timeoutRef.current) clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => {
      if (retryCountRef.current < 1) {
        // Retry once
        retryCountRef.current++
        setSendState('retrying')
        wsClient.send(env)
        timeoutRef.current = setTimeout(() => {
          setSendState('error')
          setErrorMsg(t('choice.timeout'))
        }, RESPONSE_TIMEOUT_MS)
      } else {
        setSendState('error')
        setErrorMsg(t('choice.timeout'))
      }
    }, RESPONSE_TIMEOUT_MS)
  }

  function handleChoiceClick(index: number) {
    if (sendState === 'sending' || sendState === 'retrying') return
    setActiveIndex(index)
    setSendState('sending')
    retryCountRef.current = 0
    sendResponse({
      tool_use_id: inquiry.tool_use_id,
      choice_index: index,
    })
  }

  function handleFreeTextSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!freeText.trim() || sendState === 'sending') return
    setSendState('sending')
    retryCountRef.current = 0
    sendResponse({
      tool_use_id: inquiry.tool_use_id,
      free_text: freeText.trim(),
    })
  }

  function handleCancel() {
    wsClient.send(
      makeEnvelope('inquiry-response', {
        tool_use_id: inquiry.tool_use_id,
        cancel: true,
      }),
    )
    onResolved()
  }

  // IG4: per-tool_use_id subscription — no global callback swap / chain leak
  useEffect(() => {
    const unsubscribe = wsClient.subscribeAck(inquiry.tool_use_id, {
      onAck: (_ackPayload) => {
        if (timeoutRef.current) clearTimeout(timeoutRef.current)
        onResolved()
      },
      onError: (errPayload) => {
        if (timeoutRef.current) clearTimeout(timeoutRef.current)
        setSendState('error')
        // IG1: switch-on-reason for typed i18n keys
        switch (errPayload.reason) {
          case 'send-keys-failed':
            setErrorMsg(t('choice.error.sendKeysFailed'))
            break
          case 'dialog-not-ready':
            setErrorMsg(t('choice.error.dialogNotReady'))
            break
          case 'inquiry-stale':
            setErrorMsg(t('choice.error.inquiryStale'))
            break
          case 'validation':
            setErrorMsg(t('choice.error.validation'))
            break
          default:
            setErrorMsg(t('choice.error.sendKeysFailed'))
        }
      },
    })

    return () => {
      unsubscribe()
    }
  }, [inquiry.tool_use_id, onResolved])

  const isSending = sendState === 'sending' || sendState === 'retrying'

  return (
    <main className={styles.container}>
      {/* Header bar with mode indicator */}
      <header className={styles.header}>
        <h2 className={styles.headerTitle}>Claude Code 질문</h2>
        <button
          type="button"
          className={styles.modeButton}
          onClick={onOpenModeToggle}
          aria-label={`현재 모드: ${permissionMode}. 변경하려면 클릭`}
        >
          <span
            className={`${styles.modeBadge} ${styles[`modeBadge--${permissionMode.replace('-', '')}`]}`}
          >
            {t(`modeLabel.${permissionMode as 'plan' | 'accept-edits' | 'default'}`)}
          </span>
        </button>
      </header>

      <div className={styles.scrollArea}>
        {/* Prompt context */}
        <section className={styles.contextSection}>
          <PromptContextBlock inquiry={inquiry} showMultiSelectWarning={isMultiSelect} />
        </section>

        {/* Choice buttons */}
        {!isMultiSelect && options.length > 0 && (
          <section className={styles.choicesSection}>
            <ul className={styles.choicesList}>
              {options.map((option) => (
                <li key={option.index}>
                  <ChoiceButton
                    index={option.index}
                    label={option.label}
                    description={option.description}
                    disabled={isSending}
                    loading={activeIndex === option.index && isSending}
                    onClick={handleChoiceClick}
                  />
                </li>
              ))}
            </ul>
          </section>
        )}

        {/* Free-text section */}
        <div className={styles.freeTextSection}>
          <button
            type="button"
            className={styles.freeTextToggle}
            onClick={() => setFreeTextExpanded(!freeTextExpanded)}
            aria-expanded={freeTextExpanded}
            aria-controls="free-text-area"
            disabled={isSending}
          >
            <span>{t('choice.freeTextToggle')}</span>
            <span className={styles.toggleArrow} aria-hidden="true">
              {freeTextExpanded ? '▲' : '▼'}
            </span>
          </button>

          {freeTextExpanded && (
            <form
              id="free-text-area"
              className={styles.freeTextForm}
              onSubmit={handleFreeTextSubmit}
            >
              <textarea
                className={styles.freeTextarea}
                value={freeText}
                onChange={(e) => setFreeText(e.target.value)}
                placeholder={t('choice.freeTextPlaceholder')}
                maxLength={MAX_FREE_TEXT_LENGTH}
                rows={4}
                disabled={isSending}
                aria-label={t('choice.freeTextPlaceholder')}
              />
              <div className={styles.freeTextActions}>
                <span className={styles.charCount}>
                  {freeText.length}/{MAX_FREE_TEXT_LENGTH}
                </span>
                <button
                  type="submit"
                  className={styles.sendButton}
                  disabled={isSending || !freeText.trim()}
                >
                  {isSending ? t('choice.sending') : t('choice.sendButton')}
                </button>
              </div>
            </form>
          )}
        </div>

        {/* Status / error */}
        {sendState === 'error' && (
          <div className={styles.errorBanner} role="alert">
            <span>{errorMsg}</span>
          </div>
        )}

        {sendState === 'retrying' && (
          <output className={styles.retryBanner}>{t('choice.retrying')}</output>
        )}
      </div>

      {/* Cancel button */}
      <footer className={styles.footer}>
        <button
          type="button"
          className={styles.cancelButton}
          onClick={handleCancel}
          disabled={isSending}
        >
          {t('choice.cancelButton')}
        </button>
      </footer>
    </main>
  )
}
