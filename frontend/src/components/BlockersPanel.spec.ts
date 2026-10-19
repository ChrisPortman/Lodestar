import { describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Action, Blocker, Project } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const bob = { id: 'b', version: 1, display_name: 'Bob' }

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

const deploy = {
  id: 'x1',
  project_id: 'p1',
  project_code: 'WEBS',
  reference: 'A-001',
  description: 'Deploy',
  assignee: null,
  due_date: null,
  closed: false,
  blocked_by: [],
  version: 1,
} as Action

function blocker(fields: Partial<Blocker> & Pick<Blocker, 'id' | 'reference' | 'description'>) {
  return {
    project_id: 'p1',
    project_code: 'WEBS',
    accountable: null,
    cleared: false,
    blocked_actions: [],
    version: 1,
    ...fields,
  } as Blocker
}

async function panel() {
  return screen.findByRole('region', { name: 'Blockers' })
}

async function openBlockers() {
  const list = within(await panel()).getByRole('list', { name: 'Open Blockers' })
  return within(list)
    .queryAllByRole('listitem')
    .filter((li) => li.parentElement === list)
}

/** Renders the meeting view with Ada leading and Bob a Member. */
async function meeting(blockers: Blocker[], fields: Partial<Project> = {}) {
  const view = await renderApp(
    '/projects/p1',
    [ada, bob],
    'a',
    [project(fields)],
    [],
    [deploy],
    blockers,
  )
  view.backend.members.get('p1')!.add('b')
  return view
}

function actionRow() {
  const list = screen.getByRole('list', { name: 'Open Actions' })
  return within(list).getAllByRole('listitem')[0]!
}

describe('Blockers panel', () => {
  it('sits between Actions and Risks and collapses cleared Blockers', async () => {
    await meeting([
      blocker({ id: 'k1', reference: 'B-001', description: 'No env' }),
      blocker({ id: 'k2', reference: 'B-002', description: 'Old', cleared: true }),
    ])
    await waitFor(async () => expect(await openBlockers()).toHaveLength(1))
    const order = screen.getAllByRole('region').map((r) => r.getAttribute('aria-labelledby'))
    expect(order).toEqual(['panel-actions', 'panel-blockers', 'panel-risks'])
    const cleared = within(await panel()).getByText('Cleared (1)')
    expect(cleared.closest('details')!.open).toBe(false)
  })

  it('adds a Blocker and sets who is accountable', async () => {
    const { backend } = await meeting([])
    await userEvent.click(within(await panel()).getByRole('button', { name: 'Add Blocker' }))
    await userEvent.type(within(await panel()).getByLabelText('Description'), 'Vendor late')
    await userEvent.click(within(await panel()).getByRole('button', { name: 'Add' }))
    await waitFor(async () => expect(await openBlockers()).toHaveLength(1))
    const [row] = await openBlockers()
    expect(row!.querySelector('[data-unassigned]')).not.toBeNull()

    await userEvent.selectOptions(
      await within(row!).findByLabelText('Accountable for B-001'),
      await within(row!).findByRole('option', { name: 'Bob' }),
    )
    await waitFor(() => expect(backend.blockers[0]?.accountable?.id).toBe('b'))
  })

  it('links and unlinks an Action, which shows it as blocked while open', async () => {
    const { backend } = await meeting([
      blocker({ id: 'k1', reference: 'B-001', description: 'No env' }),
    ])
    await waitFor(async () => expect(await openBlockers()).toHaveLength(1))
    const [row] = await openBlockers()
    await userEvent.selectOptions(
      within(row!).getByLabelText('Link an Action to B-001'),
      await within(row!).findByRole('option', { name: 'A-001' }),
    )
    await waitFor(() => expect(actionRow().textContent).toContain('Blocked by B-001'))
    expect(backend.blockers[0]?.blocked_actions.map((a) => a.reference)).toEqual(['A-001'])

    await userEvent.click(within(row!).getByRole('button', { name: 'Clear B-001' }))
    await waitFor(() => expect(actionRow().querySelector('[data-blocked]')).toBeNull())

    const cleared = within(await panel()).getByRole('list', { name: 'Cleared Blockers' })
    await userEvent.click(within(cleared).getByRole('button', { name: 'Unlink A-001 from B-001' }))
    await waitFor(() => expect(backend.blockers[0]?.blocked_actions).toEqual([]))
    await userEvent.click(within(cleared).getByRole('button', { name: 'Reopen B-001' }))
    await waitFor(() => expect(backend.blockers[0]?.cleared).toBe(false))
  })

  it('is read-only in an archived Project', async () => {
    await meeting([blocker({ id: 'k1', reference: 'B-001', description: 'No env' })], {
      archived: true,
    })
    await waitFor(async () => expect(await openBlockers()).toHaveLength(1))
    const [row] = await openBlockers()
    expect(within(row!).queryByRole('button', { name: 'Clear B-001' })).toBeNull()
    expect(within(row!).queryByLabelText('Link an Action to B-001')).toBeNull()
    expect(within(await panel()).queryByRole('button', { name: 'Add Blocker' })).toBeNull()
  })
})
