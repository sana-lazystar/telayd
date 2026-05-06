/**
 * PairingErrorView (FE-pwa-4)
 * Token mismatch or connection timeout
 */

import { t } from '../lib/i18n'
import styles from './PairingErrorView.module.css'

interface Props {
  reason: string | null
  onRepair: () => void
}

export function PairingErrorView({ reason, onRepair }: Props) {
  const errorMessage = reason === 'token-mismatch'
    ? t('pairingError.tokenMismatch')
    : reason === 'expired'
    ? t('pairingError.tokenMismatch')
    : t('pairingError.connectionFailed')

  return (
    <div className={styles.container} role="main">
      <div className={styles.content}>
        <div className={styles.icon} aria-hidden="true">✕</div>
        <h2 className={styles.title}>{t('pairingError.title')}</h2>
        <p className={styles.message}>{errorMessage}</p>
        <button
          className={styles.repairButton}
          onClick={onRepair}
          type="button"
        >
          {t('pairingError.repairButton')}
        </button>
      </div>
    </div>
  )
}
