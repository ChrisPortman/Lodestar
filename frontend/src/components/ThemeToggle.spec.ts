import { afterEach, describe, expect, it, vi } from 'vitest'
import { screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

const ada = { id: 'a', version: 1, display_name: 'Ada' }

function prefersDark(dark: boolean) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: dark && query === '(prefers-color-scheme: dark)',
    media: query,
    addEventListener() {},
    removeEventListener() {},
  }))
}

describe('Dark mode toggle', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
    delete document.documentElement.dataset.theme
  })

  it('follows the OS preference by default', async () => {
    prefersDark(true)
    await renderApp('/projects', [ada], 'a')
    const toggle = await screen.findByRole('button', { name: 'Dark mode' })
    expect(toggle).toHaveProperty('ariaPressed', 'true')
    expect(document.documentElement.dataset.theme).toBe('dark')
  })

  it('switches theme and remembers the choice in this browser', async () => {
    prefersDark(false)
    await renderApp('/projects', [ada], 'a')
    const toggle = await screen.findByRole('button', { name: 'Dark mode' })
    expect(toggle).toHaveProperty('ariaPressed', 'false')

    await userEvent.click(toggle)
    expect(toggle).toHaveProperty('ariaPressed', 'true')
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(localStorage.getItem('lodestar-theme')).toBe('dark')
  })

  it('a remembered choice overrides the OS preference', async () => {
    prefersDark(true)
    localStorage.setItem('lodestar-theme', 'light')
    await renderApp('/', [ada])
    const toggle = await screen.findByRole('button', { name: 'Dark mode' })
    expect(toggle).toHaveProperty('ariaPressed', 'false')
    expect(document.documentElement.dataset.theme).toBe('light')
  })
})
