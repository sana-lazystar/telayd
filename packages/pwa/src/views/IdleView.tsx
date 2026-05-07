/**
 * IdleView (FE-pwa-4)
 * Connected and waiting for inquiries
 */

import { t } from '../lib/i18n'
import type { PermissionMode } from '../lib/protocol'
import styles from './IdleView.module.css'

// IG11: literal class map — replaces fragile string-mangled `.replace('-', '')` lookup
const MODE_BADGE_CLASS: Record<PermissionMode, string> = {
  plan: styles['modeBadge--plan'],
  'accept-edits': styles['modeBadge--acceptedits'],
  default: styles['modeBadge--default'],
}

interface Props {
  permissionMode: PermissionMode
  serverVersion?: string
  onOpenModeToggle: () => void
}

export function IdleView({ permissionMode, serverVersion, onOpenModeToggle }: Props) {
  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.statusDot} aria-hidden="true" />
          <span className={styles.statusText}>{t('idle.connectedAs')}</span>
          {serverVersion && <code className={styles.versionBadge}>v{serverVersion}</code>}
        </div>
        <button
          type="button"
          className={styles.modeButton}
          onClick={onOpenModeToggle}
          aria-label={`${t('idle.mode')}: ${permissionMode}. ${t('idle.changeMode')}`}
        >
          <span className={`${styles.modeBadge} ${MODE_BADGE_CLASS[permissionMode]}`}>
            {t(`modeLabel.${permissionMode as 'plan' | 'accept-edits' | 'default'}`)}
          </span>
        </button>
      </header>

      <main className={styles.content}>
        <div className={styles.emptyState}>
          <div className={styles.emptyIcon} aria-hidden="true">
            ◇
          </div>
          <h2 className={styles.emptyTitle}>{t('idle.title')}</h2>
          <p className={styles.emptyMessage}>{t('idle.message')}</p>
        </div>
      </main>
    </div>
  )
}
