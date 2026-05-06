/**
 * PairingView (FE-pwa-1)
 * Pairing screen: token + URL form + auto-detect from fragment
 */

import { PairingForm } from '../components/PairingForm'
import { t } from '../lib/i18n'
import styles from './PairingView.module.css'

interface Props {
  onConnect: (url: string, token: string) => void
  loading?: boolean
}

export function PairingView({ onConnect, loading }: Props) {
  return (
    <div className={styles.container} role="main">
      <header className={styles.header}>
        <h1 className={styles.title}>{t('pairing.title')}</h1>
        <p className={styles.subtitle}>{t('pairing.subtitle')}</p>
      </header>

      <main className={styles.content}>
        <PairingForm onConnect={onConnect} loading={loading} />
      </main>

      <footer className={styles.footer}>
        <p className={styles.footerText}>
          <code className={styles.footerCode}>telayd start</code> 후 터미널에 출력된 URL과 토큰을 입력하세요
        </p>
      </footer>
    </div>
  )
}
