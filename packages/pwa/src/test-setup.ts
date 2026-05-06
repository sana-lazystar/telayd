/**
 * Vitest setup file
 * - Enables @testing-library/react auto cleanup between tests
 */
import { afterEach } from 'vitest'
import { cleanup } from '@testing-library/react'

afterEach(() => {
  cleanup()
})
