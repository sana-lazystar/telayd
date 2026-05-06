/**
 * PromptContextBlock — displays inquiry header/question (FE-pwa-2)
 * Monospace rendering, textContent only (no innerHTML — security)
 */

import type { InquiryPushPayload } from '../lib/protocol'
import styles from './PromptContextBlock.module.css'

interface Props {
  inquiry: InquiryPushPayload
  showMultiSelectWarning?: boolean
}

export function PromptContextBlock({ inquiry, showMultiSelectWarning }: Props) {
  const question = inquiry.questions[0]

  return (
    <div className={styles.contextBlock} role="region" aria-label="Claude Code 질문">
      {/* Header */}
      {inquiry.header && (
        <div className={styles.header}>
          <span className={styles.headerIcon} aria-hidden="true">◆</span>
          <span className={styles.headerText}>{inquiry.header}</span>
        </div>
      )}

      {/* Question text */}
      {question && (
        <div className={styles.questionBox}>
          {/* textContent only — no dangerouslySetInnerHTML */}
          <p className={styles.questionText}>{question.question}</p>
          {showMultiSelectWarning && (
            <p className={styles.warning} role="alert">
              복수 선택 질문은 수동으로 응답해주세요
            </p>
          )}
        </div>
      )}

      {/* Session metadata */}
      <div className={styles.meta}>
        <span className={styles.metaItem} title="tool_use_id">
          <span className={styles.metaIcon} aria-hidden="true">#</span>
          <code className={styles.metaCode}>{inquiry.tool_use_id.slice(-8)}</code>
        </span>
        {inquiry.tmux_session && (
          <span className={styles.metaItem} title="tmux session">
            <span className={styles.metaIcon} aria-hidden="true">⊡</span>
            <code className={styles.metaCode}>{inquiry.tmux_session}</code>
          </span>
        )}
      </div>
    </div>
  )
}
