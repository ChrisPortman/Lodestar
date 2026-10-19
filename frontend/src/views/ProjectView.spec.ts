import { describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Project } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }

function project(fields: Partial<Project> = {}) {
  return {
    id: 'p1',
    name: 'Website',
    project_code: 'WEBS',
    description: null,
    archived: false,
    project_leader: { id: 'a', display_name: 'Ada' },
    health_rating: null,
    version: 1,
    ...fields,
  } as Project
}

async function badge() {
  return screen.findByRole('status', { name: 'Health Rating' })
}

async function openEditor() {
  await userEvent.click(await screen.findByRole('button', { name: 'Edit Health Rating' }))
  return screen.getByRole('group', { name: 'Health Rating' })
}

describe('Health Rating in the meeting view header', () => {
  it('shows a Project that is not rated', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project()])
    expect(await screen.findByRole('heading', { name: /Website/ })).toBeTruthy()
    expect((await badge()).textContent).toContain('Not rated')
  })

  it('sets a rating with a comment', async () => {
    const { backend } = await renderApp('/projects/p1', [ada], 'a', [project()])
    const editor = await openEditor()
    await userEvent.click(within(editor).getByLabelText('Amber'))
    await userEvent.type(within(editor).getByLabelText('Comment'), 'Vendor late')
    await userEvent.click(within(editor).getByRole('button', { name: 'Save' }))

    await waitFor(() => expect(screen.queryByRole('group', { name: 'Health Rating' })).toBeNull())
    expect(backend.projects[0]!.health_rating).toEqual({ rating: 'amber', comment: 'Vendor late' })
    await waitFor(async () => expect((await badge()).textContent).toContain('Amber'))
    expect(screen.getByText('Vendor late')).toBeTruthy()
  })

  it('changes the rating and clears the comment', async () => {
    const rated = project({ health_rating: { rating: 'red', comment: 'Behind' } })
    const { backend } = await renderApp('/projects/p1', [ada], 'a', [rated])
    expect((await badge()).textContent).toContain('Red')
    const editor = await openEditor()
    await userEvent.click(within(editor).getByLabelText('Green'))
    await userEvent.clear(within(editor).getByLabelText('Comment'))
    await userEvent.click(within(editor).getByRole('button', { name: 'Save' }))

    await waitFor(() =>
      expect(backend.projects[0]!.health_rating).toEqual({ rating: 'green', comment: null }),
    )
    await waitFor(async () => expect((await badge()).textContent).toContain('Green'))
    expect(screen.queryByText('Behind')).toBeNull()
  })

  it('can go back to not rated', async () => {
    const rated = project({ health_rating: { rating: 'red', comment: 'Behind' } })
    const { backend } = await renderApp('/projects/p1', [ada], 'a', [rated])
    const editor = await openEditor()
    await userEvent.click(within(editor).getByLabelText('Not rated'))
    await userEvent.click(within(editor).getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(backend.projects[0]!.health_rating).toBeNull())
    await waitFor(async () => expect((await badge()).textContent).toContain('Not rated'))
  })

  it('cancelling leaves the rating unchanged', async () => {
    const { backend } = await renderApp('/projects/p1', [ada], 'a', [project()])
    const editor = await openEditor()
    await userEvent.click(within(editor).getByLabelText('Red'))
    await userEvent.click(within(editor).getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByRole('group', { name: 'Health Rating' })).toBeNull()
    expect(backend.projects[0]!.version).toBe(1)
    expect((await badge()).textContent).toContain('Not rated')
  })

  it('an archived Project cannot be rated', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project({ archived: true })])
    await badge()
    expect(screen.queryByRole('button', { name: 'Edit Health Rating' })).toBeNull()
  })
})
