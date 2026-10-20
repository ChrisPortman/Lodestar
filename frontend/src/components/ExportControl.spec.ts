import { describe, expect, it } from 'vitest'
import { screen } from '@testing-library/vue'
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

async function openExport() {
  await userEvent.click(await screen.findByRole('button', { name: 'Project menu' }))
  await userEvent.click(screen.getByRole('button', { name: 'Export' }))
}

describe('Project Export on the project page', () => {
  it('downloads the export without history by default', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project])
    await openExport()
    const link = screen.getByRole('link', { name: 'Export' })
    expect(link.getAttribute('href')).toBe('/api/v1/projects/p1/export')
    expect(link.hasAttribute('download')).toBe(true)
  })

  it('includes history when the box is ticked', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project])
    await openExport()
    await userEvent.click(screen.getByLabelText('Include history'))
    expect(screen.getByRole('link', { name: 'Export' }).getAttribute('href')).toBe(
      '/api/v1/projects/p1/export?include_history=true',
    )
  })
})
