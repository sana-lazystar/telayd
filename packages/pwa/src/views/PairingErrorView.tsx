/**
 * PairingErrorView (FE-pwa-4)
 * Token mismatch or connection timeout
 */

import { useEffect } from 'react'
import { t } from '../lib/i18n'
import { clearStorage } from '../lib/storage'
import styles from './PairingErrorView.module.css'

interface Props {
  reason: string | null
  onRepair: () => void
}

export function PairingErrorView({ reason, onRepair }: Props) {
  // IG11: clear stale localStorage on pairing error mount so PairingForm starts fresh
  useEffect(() => {
    clearStorage()
  }, [])

  // IG4: each PairingRejectReason variant now has its own translation key.
  // 'replaced' → "다른 기기에서 연결됨" (was incorrectly showing tokenMismatch UX)
  // 'expired'  → dedicated expiry message (was also incorrectly showing tokenMismatch)
  let errorMessage: string
  switch (reason) {
    case 'token-mismatch':
      errorMessage = t('pairingError.tokenMismatch')
      break
    case 'replaced':
      errorMessage = t('pairingError.replaced')
      break
    case 'expired':
      errorMessage = t('pairingError.expired')
      break
    default:
      errorMessage = t('pairingError.connectionFailed')
  }

  return (
    <main className={styles.container}>
      <div className={styles.content}>
        <div className={styles.icon} aria-hidden="true">
          ✕
        </div>
        <h2 className={styles.title}>{t('pairingError.title')}</h2>
        <p className={styles.message}>{errorMessage}</p>
        <button className={styles.repairButton} onClick={onRepair} type="button">
          {t('pairingError.repairButton')}
        </button>
      </div>
    </main>
  )
}
