import { cleanup } from '@testing-library/react'
/**
 * Vitest setup file
 * - Enables @testing-library/react auto cleanup between tests
 */
import { afterEach } from 'vitest'

afterEach(() => {
  cleanup()
})
