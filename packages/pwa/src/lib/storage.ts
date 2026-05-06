/**
 * localStorage utilities (FE-pwa-1 dependency)
 * Security: full token NEVER stored — only masked form
 * architecture.md §8.3 + security-guidelines.md §4.4
 */

const LAST_URL_KEY = 'telayd:last_tunnel_url'
const LAST_TOKEN_MASKED_KEY = 'telayd:last_token_masked'

/** Mask token for display and storage: AbCd...****  */
export function maskToken(token: string): string {
  if (token.length < 8) return '****'
  return `${token.slice(0, 4)}...${token.slice(-4)}`
}

export function saveLastUrl(url: string): void {
  try {
    localStorage.setItem(LAST_URL_KEY, url)
  } catch {
    // localStorage may be unavailable in some contexts
  }
}

export function loadLastUrl(): string {
  try {
    return localStorage.getItem(LAST_URL_KEY) ?? ''
  } catch {
    return ''
  }
}

export function saveLastTokenMasked(token: string): void {
  try {
    // Store only masked form — never the full token
    localStorage.setItem(LAST_TOKEN_MASKED_KEY, maskToken(token))
  } catch {
    // localStorage may be unavailable
  }
}

export function clearStorage(): void {
  try {
    localStorage.removeItem(LAST_URL_KEY)
    localStorage.removeItem(LAST_TOKEN_MASKED_KEY)
  } catch {
    // localStorage may be unavailable
  }
}
