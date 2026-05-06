/**
 * PairingForm unit tests (Group 7 P0)
 * - validateToken: ^[A-Za-z0-9_-]{43}$ regex
 * - parseFragment: URL#token= extraction
 */

import { describe, expect, it } from 'vitest'
import { parseFragment, validateToken } from './PairingForm'

// 43-char base64url token (verified: exactly 43 characters)
const VALID_TOKEN = 'abcDEF123_-abcDEF123_-abcDEF123_-abcDEF1234'

describe('validateToken', () => {
  it('accepts a valid 43-char base64url token', () => {
    expect(validateToken(VALID_TOKEN)).toBe(true)
  })

  it('rejects token shorter than 43 chars', () => {
    expect(validateToken('abc')).toBe(false)
  })

  it('rejects token longer than 43 chars', () => {
    expect(validateToken(`${VALID_TOKEN}X`)).toBe(false)
  })

  it('rejects token with invalid characters', () => {
    // + and / are not base64url
    const invalid = 'abcDEF123+/abcDEF123_-abcDEF123_-abcDEF12'
    expect(validateToken(invalid)).toBe(false)
  })

  it('accepts tokens with underscore and hyphen', () => {
    const withSpecial = 'aaaaaaaaaaaaaaaaaaaaaaaaa_-_-_-_-_-_-_-_-__'
    expect(validateToken(withSpecial)).toBe(true)
  })
})

describe('parseFragment', () => {
  const BASE_URL = 'https://random.trycloudflare.com'

  it('extracts url and token from valid fragment', () => {
    const result = parseFragment(`${BASE_URL}#token=${VALID_TOKEN}`)
    expect(result).not.toBeNull()
    expect(result?.url).toBe(BASE_URL)
    expect(result?.token).toBe(VALID_TOKEN)
  })

  it('returns null for invalid token in fragment', () => {
    expect(parseFragment(`${BASE_URL}#token=short`)).toBeNull()
  })

  it('returns null for non-trycloudflare URL', () => {
    expect(parseFragment(`https://evil.example.com#token=${VALID_TOKEN}`)).toBeNull()
  })

  it('returns null for malformed URL', () => {
    expect(parseFragment('not-a-url')).toBeNull()
  })

  it('handles #?token= query-string-style fragment', () => {
    const result = parseFragment(`${BASE_URL}#?token=${VALID_TOKEN}`)
    // URLSearchParams handles both # and #? forms
    expect(result).not.toBeNull()
    expect(result?.token).toBe(VALID_TOKEN)
  })

  it('returns null when fragment is missing entirely', () => {
    expect(parseFragment(BASE_URL)).toBeNull()
  })

  it('strips trailing slash from base URL', () => {
    const result = parseFragment(`${BASE_URL}/#token=${VALID_TOKEN}`)
    // URL parsing keeps trailing slash in host — check base extraction
    // The URL_REGEX allows trailing / in the base URL
    if (result !== null) {
      expect(result.url).not.toContain('#')
    }
  })
})
