/**
 * DisconnectedView (FE-pwa-4)
 * Reconnecting state with attempt counter
 */

import { t } from '../lib/i18n'
import styles from './DisconnectedView.module.css'

interface Props {
  attempt: number
  onReconnect: () => void
}

export function DisconnectedView({ attempt, onReconnect }: Props) {
  return (
    <div className={styles.container} role="main">
      <div className={styles.content}>
        <div className={styles.icon} aria-hidden="true">
          <span className={styles.iconSpinner} />
        </div>
        <h2 className={styles.title}>{t('disconnected.title')}</h2>
        <p className={styles.message}>{t('disconnected.message')}</p>
        {attempt > 0 && (
          <p className={styles.attempt} role="status">
            {t('disconnected.retrying', { attempt })}
          </p>
        )}
        <button
          className={styles.reconnectButton}
          onClick={onReconnect}
          type="button"
        >
          {t('disconnected.reconnectNow')}
        </button>
      </div>
    </div>
  )
}
