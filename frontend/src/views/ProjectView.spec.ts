import { describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Project } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const bob = { id: 'b', version: 1, display_name: 'Bob' }

const web = {
  id: 'p1',
  name: 'Website',
  project_code: 'WEBS',
  description: null,
  archived: false,
  project_leader: { id: 'a', display_name: 'Ada' },
  version: 1,
} as Project

async function openMembers() {
  await userEvent.click(await screen.findByRole('button', { name: 'Members' }))
  return screen.getByRole('dialog', { name: 'Members' })
}

function memberNames(panel: HTMLElement) {
  return within(within(panel).getByRole('list', { name: 'Members' }))
    .queryAllByRole('listitem')
    .map((li) => li.textContent ?? '')
}

describe('Project meeting view', () => {
  it('shows the code, name and Acting Person in the header', async () => {
    await renderApp('/projects/p1', [ada], 'a', [web])
    const header = await screen.findByRole('banner', { name: 'Project' })
    expect(within(header).getByText('WEBS')).toBeTruthy()
    expect(within(header).getByRole('heading', { name: /Website/ })).toBeTruthy()
    expect(within(header).getByText(/Acting as/).textContent).toContain('Ada')
    expect(within(header).queryByText('No Project Leader')).toBeNull()
  })

  it('flags a Project with no Project Leader', async () => {
    await renderApp('/projects/p1', [ada], 'a', [{ ...web, project_leader: null }])
    expect(await screen.findByText('No Project Leader')).toBeTruthy()
  })

  it('lists Members with a leader badge and adds one from People', async () => {
    const { backend } = await renderApp('/projects/p1', [ada, bob], 'a', [web])
    const panel = await openMembers()
    await waitFor(() => expect(memberNames(panel)).toHaveLength(1))
    expect(memberNames(panel)[0]).toContain('Leader')

    await userEvent.selectOptions(within(panel).getByLabelText('Add member'), 'b')
    await userEvent.click(within(panel).getByRole('button', { name: 'Add' }))
    await waitFor(() => expect(memberNames(panel)).toHaveLength(2))
    expect(backend.projects[0]?.version).toBe(2)
  })

  it('makes another Member the Project Leader', async () => {
    const { backend } = await renderApp('/projects/p1', [ada, bob], 'a', [web])
    backend.members.get('p1')?.add('b')
    const panel = await openMembers()
    await userEvent.click(await within(panel).findByRole('button', { name: 'Make Bob leader' }))
    await waitFor(() => expect(backend.projects[0]?.project_leader?.id).toBe('b'))
  })

  it('removing the leader leaves the Project flagged with no leader', async () => {
    await renderApp('/projects/p1', [ada, bob], 'a', [web])
    const panel = await openMembers()
    await userEvent.click(await within(panel).findByRole('button', { name: 'Remove Ada' }))
    expect(await screen.findByText('No Project Leader')).toBeTruthy()
    await waitFor(() => expect(memberNames(panel)).toHaveLength(0))
  })
})
