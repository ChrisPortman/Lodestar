import { describe, expect, it } from 'vitest'
import { screen, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Project } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const project = {
  id: 'p1',
  name: 'Website',
  project_code: 'WEBS',
  description: null,
  archived: false,
  project_leader: { id: 'a', display_name: 'Ada' },
  health_rating: null,
  version: 1,
} as Project

async function openMenu() {
  const burger = await screen.findByRole('button', { name: 'Project menu' })
  await userEvent.click(burger)
  return { burger, panel: screen.getByRole('dialog', { name: 'Project menu' }) }
}

describe('Project menu (#61)', () => {
  it('keeps Members and Export out of the header', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project])
    const header = (await screen.findByRole('heading', { name: /Website/ })).closest('header')!
    expect(within(header).queryByRole('button', { name: 'Members' })).toBeNull()
    expect(within(header).queryByRole('link', { name: 'Export' })).toBeNull()
    expect(within(header).getByRole('link', { name: 'Project on a Page' })).toBeTruthy()
  })

  it('opens on the menu and goes back from a view', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project])
    const { burger, panel } = await openMenu()
    expect(burger.getAttribute('aria-expanded')).toBe('true')
    await userEvent.click(within(panel).getByRole('button', { name: 'Manage Members' }))
    expect(screen.getByRole('dialog', { name: 'Members' })).toBeTruthy()
    await userEvent.click(screen.getByRole('button', { name: 'Back' }))
    expect(screen.getByRole('button', { name: 'Export' })).toBeTruthy()
  })

  it('reopens on the menu after closing from a view', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project])
    const { panel } = await openMenu()
    await userEvent.click(within(panel).getByRole('button', { name: 'Export' }))
    await userEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    await openMenu()
    expect(screen.getByRole('button', { name: 'Manage Members' })).toBeTruthy()
  })

  it('closes on Escape', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project])
    const { panel } = await openMenu()
    within(panel).getByRole('button', { name: 'Close' }).focus()
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})
