/**
 * PairingForm component (FE-pwa-1)
 * - token format validation: ^[A-Za-z0-9_-]{43}$
 * - URL fragment auto-extraction
 * - localStorage last-url caching
 * Security: token masked, history.replaceState removes fragment
 */

import { useEffect, useRef, useState } from 'react'
import { t } from '../lib/i18n'
import { loadLastUrl, maskToken, saveLastTokenMasked, saveLastUrl } from '../lib/storage'
import styles from './PairingForm.module.css'

const TOKEN_REGEX = /^[A-Za-z0-9_-]{43}$/
const URL_REGEX = /^https:\/\/[a-z0-9-]{1,63}\.trycloudflare\.com\/?$/

export function parseFragment(href: string): { url: string; token: string } | null {
  let parsed: URL
  try {
    parsed = new URL(href)
  } catch {
    return null
  }
  const baseUrl = `${parsed.protocol}//${parsed.host}`
  if (!URL_REGEX.test(baseUrl)) return null
  // Fragment may be #token=... or #?token=...
  const rawFragment = parsed.hash.startsWith('#') ? parsed.hash.slice(1) : parsed.hash
  const params = new URLSearchParams(rawFragment)
  const token = params.get('token')
  if (!token || !TOKEN_REGEX.test(token)) return null
  return { url: baseUrl, token }
}

export function validateToken(token: string): boolean {
  return TOKEN_REGEX.test(token)
}

export function validateUrl(url: string): boolean {
  const cleaned = url.replace(/\/$/, '')
  return URL_REGEX.test(cleaned)
}

interface Props {
  onConnect: (url: string, token: string) => void
  loading?: boolean
}

export function PairingForm({ onConnect, loading = false }: Props) {
  const [url, setUrl] = useState(() => loadLastUrl())
  const [token, setToken] = useState('')
  const [urlError, setUrlError] = useState('')
  const [tokenError, setTokenError] = useState('')
  const [autoDetected, setAutoDetected] = useState(false)
  const tokenInputRef = useRef<HTMLInputElement>(null)

  // Auto-extract from URL fragment on mount
  useEffect(() => {
    const result = parseFragment(window.location.href)
    if (result) {
      setUrl(result.url)
      setToken(result.token)
      setAutoDetected(true)
      // Remove fragment from URL bar — security: prevent reload re-exposing token
      history.replaceState(null, '', window.location.pathname + window.location.search)
    }
  }, [])

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    let valid = true

    const cleanUrl = url.replace(/\/$/, '')
    if (!validateUrl(cleanUrl)) {
      setUrlError(t('pairing.urlError'))
      valid = false
    } else {
      setUrlError('')
    }

    if (!validateToken(token)) {
      setTokenError(t('pairing.tokenError'))
      valid = false
    } else {
      setTokenError('')
    }

    if (!valid) return

    // Cache URL for quick re-pairing; never store full token
    saveLastUrl(cleanUrl)
    saveLastTokenMasked(token)

    onConnect(cleanUrl, token)
  }

  return (
    <form className={styles.form} onSubmit={handleSubmit} noValidate>
      <div className={styles.fieldGroup}>
        <label className={styles.label} htmlFor="pairing-url">
          {t('pairing.urlLabel')}
        </label>
        <input
          id="pairing-url"
          className={`${styles.input} ${urlError ? styles.inputError : ''}`}
          type="url"
          value={url}
          onChange={(e) => {
            setUrl(e.target.value)
            setUrlError('')
          }}
          placeholder={t('pairing.urlPlaceholder')}
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          aria-describedby={urlError ? 'url-error' : undefined}
          aria-invalid={urlError ? 'true' : undefined}
          disabled={loading}
        />
        {urlError && (
          <p id="url-error" className={styles.errorText} role="alert">
            {urlError}
          </p>
        )}
        {!urlError && url && (
          <p className={styles.helperText}>
            {t('pairing.lastUrl')}: <code className={styles.code}>{url}</code>
          </p>
        )}
      </div>

      <div className={styles.fieldGroup}>
        <label className={styles.label} htmlFor="pairing-token">
          {t('pairing.tokenLabel')}
          {autoDetected && (
            <span className={styles.autoDetectedBadge}>{t('pairing.autoDetected')}</span>
          )}
        </label>
        <input
          id="pairing-token"
          ref={tokenInputRef}
          className={`${styles.input} ${styles.tokenInput} ${tokenError ? styles.inputError : ''}`}
          // Security: password type + no autocomplete + no spellcheck
          type="password"
          autoComplete="off"
          spellCheck={false}
          // IG11: mobile keyboard hints — no uppercase on token input
          inputMode="text"
          autoCapitalize="none"
          value={token}
          onChange={(e) => {
            setToken(e.target.value)
            setTokenError('')
          }}
          placeholder={t('pairing.tokenPlaceholder')}
          aria-describedby={tokenError ? 'token-error' : 'token-hint'}
          aria-invalid={tokenError ? 'true' : undefined}
          disabled={loading}
          maxLength={43}
        />
        {tokenError ? (
          <p id="token-error" className={styles.errorText} role="alert">
            {tokenError}
          </p>
        ) : (
          <p id="token-hint" className={styles.helperText}>
            {token ? maskToken(token) : '43자 base64url 형식'}
          </p>
        )}
      </div>

      <button type="submit" className={styles.submitButton} disabled={loading} aria-busy={loading}>
        {loading ? t('pairing.connecting') : t('pairing.connectButton')}
      </button>
    </form>
  )
}
