/**
 * IdleView (FE-pwa-4)
 * Connected and waiting for inquiries
 */

import type { PermissionMode } from '../lib/protocol'
import { t } from '../lib/i18n'
import styles from './IdleView.module.css'

interface Props {
  permissionMode: PermissionMode
  serverVersion?: string
  onOpenModeToggle: () => void
}

export function IdleView({ permissionMode, serverVersion, onOpenModeToggle }: Props) {
  return (
    <div className={styles.container} role="main">
      <header className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.statusDot} aria-hidden="true" />
          <span className={styles.statusText}>{t('idle.connectedAs')}</span>
          {serverVersion && (
            <code className={styles.versionBadge}>v{serverVersion}</code>
          )}
        </div>
        <button
          className={styles.modeButton}
          onClick={onOpenModeToggle}
          aria-label={`${t('idle.mode')}: ${permissionMode}. ${t('idle.changeMode')}`}
        >
          <span className={`${styles.modeBadge} ${styles[`modeBadge--${permissionMode.replace('-', '')}`]}`}>
            {t(`modeLabel.${permissionMode as 'plan' | 'accept-edits' | 'default'}`)}
          </span>
        </button>
      </header>

      <main className={styles.content}>
        <div className={styles.emptyState}>
          <div className={styles.emptyIcon} aria-hidden="true">◇</div>
          <h2 className={styles.emptyTitle}>{t('idle.title')}</h2>
          <p className={styles.emptyMessage}>{t('idle.message')}</p>
        </div>
      </main>
    </div>
  )
}
