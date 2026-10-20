import { describe, expect, it } from 'vitest'
import { DARK_QUERY, STORAGE_KEY } from './theme'

// public/theme.js runs before first paint and can't import src/theme.ts.
import preload from '../public/theme.js?raw'

describe('the pre-paint theme script', () => {
  it('reads the same storage key as the app', () => {
    expect(STORAGE_KEY).toBe('lodestar-theme')
    expect(preload).toContain(`localStorage.getItem('${STORAGE_KEY}')`)
  })

  it('follows the same OS preference query as the app', () => {
    expect(DARK_QUERY).toBe('(prefers-color-scheme: dark)')
    expect(preload).toContain(`matchMedia('${DARK_QUERY}')`)
  })
})
