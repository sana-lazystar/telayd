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
    <main className={styles.container}>
      <div className={styles.content}>
        <div className={styles.icon} aria-hidden="true">
          <span className={styles.iconSpinner} />
        </div>
        <h2 className={styles.title}>{t('disconnected.title')}</h2>
        <p className={styles.message}>{t('disconnected.message')}</p>
        {attempt > 0 && (
          <output className={styles.attempt}>{t('disconnected.retrying', { attempt })}</output>
        )}
        <button className={styles.reconnectButton} onClick={onReconnect} type="button">
          {t('disconnected.reconnectNow')}
        </button>
      </div>
    </main>
  )
}
