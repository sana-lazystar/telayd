/**
 * i18n utility (FE-pwa-6)
 * Default locale: ko-KR
 * Usage: t('pairing.title'), t('choice.error', { reason: '...' })
 */

import { ko } from '../i18n/ko'

// Use unknown to avoid circular reference in type alias
function getNestedValue(obj: Record<string, unknown>, path: string): string | null {
  const keys = path.split('.')
  let current: unknown = obj
  for (const key of keys) {
    if (typeof current !== 'object' || current === null) return null
    current = (current as Record<string, unknown>)[key] ?? null
    if (current === null || current === undefined) return null
  }
  return typeof current === 'string' ? current : null
}

function interpolate(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => {
    const val = vars[key]
    return val !== undefined ? String(val) : `{${key}}`
  })
}

export function t(path: string, vars?: Record<string, string | number>): string {
  const raw = getNestedValue(ko as unknown as Record<string, unknown>, path)
  if (raw === null) {
    // Return the path as fallback for missing keys
    return path
  }
  return vars ? interpolate(raw, vars) : raw
}
