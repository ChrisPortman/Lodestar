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
  health_rating: null,
  version: 1,
} as Project

async function openMembers() {
  await userEvent.click(await screen.findByRole('button', { name: 'Project menu' }))
  await userEvent.click(screen.getByRole('button', { name: 'Manage Members' }))
  return screen.getByRole('dialog', { name: 'Members' })
}

function memberNames(panel: HTMLElement) {
  return within(within(panel).getByRole('list', { name: 'Members' }))
    .queryAllByRole('listitem')
    .map((li) => li.textContent ?? '')
}

describe('Members in the meeting view', () => {
  it('shows the code, name and Acting Person in the header', async () => {
    await renderApp('/projects/p1', [ada], 'a', [web])
    const heading = await screen.findByRole('heading', { name: /Website/ })
    expect(heading.textContent).toContain('WEBS')
    const header = heading.closest('header')!
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

  it("shows each Member's open Actions and Blockers", async () => {
    const item = { project_id: 'p1', project_code: 'WEBS', version: 1 }
    const toBob = { id: 'b', display_name: 'Bob' }
    const { backend } = await renderApp(
      '/projects/p1',
      [ada, bob],
      'a',
      [web],
      [],
      [
        {
          ...item,
          id: 'x1',
          reference: 'A-001',
          description: 'a',
          assignee: toBob,
          due_date: null,
          closed: false,
          blocked_by: [],
        },
        {
          ...item,
          id: 'x2',
          reference: 'A-002',
          description: 'b',
          assignee: toBob,
          due_date: null,
          closed: true,
          blocked_by: [],
        },
      ],
      [
        {
          ...item,
          id: 'k1',
          reference: 'B-001',
          description: 'c',
          accountable: toBob,
          cleared: false,
          blocked_actions: [],
        },
      ],
    )
    backend.members.get('p1')?.add('b')
    const panel = await openMembers()
    await waitFor(() =>
      expect(memberNames(panel)).toEqual([
        expect.stringContaining('0 open Actions, 0 open Blockers'),
        expect.stringContaining('1 open Action, 1 open Blocker'),
      ]),
    )
  })

  it('removing the leader leaves the Project flagged with no leader', async () => {
    await renderApp('/projects/p1', [ada, bob], 'a', [web])
    const panel = await openMembers()
    await userEvent.click(await within(panel).findByRole('button', { name: 'Remove Ada' }))
    expect(await screen.findByText('No Project Leader')).toBeTruthy()
    await waitFor(() => expect(memberNames(panel)).toHaveLength(0))
  })

  it('adds a new Person as a Member (#59)', async () => {
    const { backend } = await renderApp('/projects/p1', [ada], 'a', [web])
    const panel = await openMembers()
    await waitFor(() => expect(memberNames(panel)).toHaveLength(1))
    await userEvent.selectOptions(within(panel).getByLabelText('Add member'), 'New Person…')
    await userEvent.type(within(panel).getByLabelText('New member'), 'Dave{Enter}')
    await waitFor(() => expect(memberNames(panel)).toHaveLength(2))
    expect(backend.people.map((p) => p.display_name)).toEqual(['Ada', 'Dave'])
  })
})
