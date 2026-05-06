/**
 * ConnectingView (FE-pwa-4)
 * Shows while WebSocket handshake is in progress
 */

import { t } from '../lib/i18n'
import styles from './ConnectingView.module.css'

export function ConnectingView() {
  return (
    <div className={styles.container} role="main" aria-label={t('connecting.title')}>
      <div className={styles.content}>
        <div className={styles.spinner} aria-hidden="true" />
        <h2 className={styles.title}>{t('connecting.title')}</h2>
        <p className={styles.message}>{t('connecting.message')}</p>
      </div>
    </div>
  )
}
