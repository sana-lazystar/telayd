/**
 * ChoiceButton — 4-button choice UI atom (FE-pwa-2)
 * Anthropic #29214/#29438 정조준
 */

import type { CSSProperties } from 'react'
import styles from './ChoiceButton.module.css'

export interface ChoiceButtonProps {
  index: number
  label: string
  description?: string
  disabled?: boolean
  loading?: boolean
  onClick: (index: number) => void
}

export function ChoiceButton({
  index,
  label,
  description,
  disabled = false,
  loading = false,
  onClick,
}: ChoiceButtonProps) {
  const handleClick = () => {
    if (!disabled && !loading) {
      onClick(index)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      handleClick()
    }
  }

  return (
    <button
      className={styles.choiceButton}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      disabled={disabled || loading}
      aria-label={description ? `${label}: ${description}` : label}
      style={{ '--btn-index': index } as CSSProperties}
    >
      <span className={styles.indexBadge} aria-hidden="true">
        {index}
      </span>
      <span className={styles.content}>
        <span className={styles.label}>{label}</span>
        {description && (
          <span className={styles.description}>{description}</span>
        )}
      </span>
      {loading && (
        <span className={styles.spinner} aria-hidden="true" />
      )}
    </button>
  )
}
