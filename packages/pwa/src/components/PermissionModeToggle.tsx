/**
 * PermissionModeToggle — radio/segmented control for permission mode (FE-pwa-5)
 * Anthropic #35637 정조준
 */

import type { PermissionMode } from '../lib/protocol'
import { t } from '../lib/i18n'
import styles from './PermissionModeToggle.module.css'

interface ModeOption {
  value: PermissionMode
  label: string
  description: string
}

const MODES: ModeOption[] = [
  {
    value: 'default',
    label: t('permissionMode.default'),
    description: t('permissionMode.defaultDesc'),
  },
  {
    value: 'accept-edits',
    label: t('permissionMode.acceptEdits'),
    description: t('permissionMode.acceptEditsDesc'),
  },
  {
    value: 'plan',
    label: t('permissionMode.plan'),
    description: t('permissionMode.planDesc'),
  },
]

interface Props {
  selected: PermissionMode
  onChange: (mode: PermissionMode) => void
  disabled?: boolean
}

export function PermissionModeToggle({ selected, onChange, disabled }: Props) {
  return (
    <div className={styles.toggleGroup} role="radiogroup" aria-label={t('permissionMode.title')}>
      {MODES.map((mode) => (
        <label
          key={mode.value}
          className={`${styles.modeOption} ${selected === mode.value ? styles.selected : ''}`}
        >
          <input
            type="radio"
            name="permission-mode"
            value={mode.value}
            checked={selected === mode.value}
            onChange={() => onChange(mode.value)}
            disabled={disabled}
            className={styles.radioInput}
          />
          <div className={styles.modeContent}>
            <div className={styles.modeHeader}>
              <span className={styles.modeLabel}>{mode.label}</span>
              {selected === mode.value && (
                <span className={styles.checkmark} aria-hidden="true">✓</span>
              )}
            </div>
            <span className={styles.modeDescription}>{mode.description}</span>
          </div>
        </label>
      ))}
    </div>
  )
}
