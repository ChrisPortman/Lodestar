import { describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Action, Project } from '../test/server'

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

function action(fields: Partial<Action> & Pick<Action, 'id' | 'reference' | 'description'>) {
  return {
    project_id: 'p1',
    project_code: 'WEBS',
    assignee: null,
    due_date: null,
    closed: false,
    version: 1,
    ...fields,
  } as Action
}

async function panel() {
  return screen.findByRole('region', { name: 'Actions' })
}

async function openActions() {
  const list = within(await panel()).getByRole('list', { name: 'Open Actions' })
  return within(list).queryAllByRole('listitem')
}

/** Renders the meeting view with Ada leading and Bob a Member. */
async function meeting(actions: Action[], fields: Partial<Project> = {}) {
  const view = await renderApp('/projects/p1', [ada, bob], 'a', [project(fields)], [], actions)
  view.backend.members.get('p1')!.add('b')
  return view
}

describe('Actions panel', () => {
  it('comes first, flags unassigned and overdue Actions, and collapses closed ones', async () => {
    await meeting([
      action({ id: 'x1', reference: 'A-001', description: 'Draft\nplan', due_date: '2001-01-01' }),
      action({
        id: 'x2',
        reference: 'A-002',
        description: 'Book room',
        assignee: { id: 'b', display_name: 'Bob' },
        due_date: '2999-01-01',
      }),
      action({ id: 'x3', reference: 'A-003', description: 'Old', closed: true }),
    ])
    await waitFor(async () => expect(await openActions()).toHaveLength(2))
    const regions = screen.getAllByRole('region')
    expect(regions[0]!.getAttribute('aria-labelledby')).toBe('panel-actions')

    const [first, second] = await openActions()
    expect(within(first!).getByText(/Draft/).textContent).toBe('Draft\nplan')
    expect(first!.querySelector('[data-unassigned]')).not.toBeNull()
    expect(
      within(first!).getByLabelText<HTMLInputElement>('Due date of A-001').dataset.overdue,
    ).toBe('true')
    expect(second!.querySelector('[data-unassigned]')).toBeNull()
    expect(
      within(second!).getByLabelText<HTMLInputElement>('Due date of A-002').dataset.overdue,
    ).toBe('false')
    const closed = within(await panel()).getByText('Closed (1)')
    expect(closed.closest('details')!.open).toBe(false)
  })

  it('adds an Action with the keyboard', async () => {
    const { backend } = await meeting([])
    const region = await panel()
    await userEvent.click(within(region).getByRole('button', { name: 'Add Action' }))
    await userEvent.type(
      within(region).getByLabelText('Description'),
      'Write{Shift>}{Enter}{/Shift}it up',
    )
    await userEvent.keyboard('{Control>}{Enter}{/Control}')
    await waitFor(async () => expect(await openActions()).toHaveLength(1))
    expect(backend.actions[0]).toMatchObject({ reference: 'A-001', description: 'Write\nit up' })
  })

  it('reassigns and sets a due date straight away', async () => {
    const { backend } = await meeting([
      action({ id: 'x1', reference: 'A-001', description: 'Draft plan' }),
    ])
    const region = await panel()
    const assignee = await within(region).findByLabelText('Assignee of A-001')
    await within(assignee).findByRole('option', { name: 'Bob' })
    await userEvent.selectOptions(assignee, 'b')
    await waitFor(() =>
      expect(backend.actions[0]).toMatchObject({ assignee: { id: 'b' }, version: 2 }),
    )
    const due = within(region).getByLabelText('Due date of A-001')
    await userEvent.type(due, '2030-05-06')
    await waitFor(() => expect(backend.actions[0]).toMatchObject({ due_date: '2030-05-06' }))
  })

  it('edits a description with an explicit Save, and Esc cancels', async () => {
    const { backend } = await meeting([
      action({ id: 'x1', reference: 'A-001', description: 'Draft plan' }),
    ])
    const region = await panel()
    await userEvent.click(await within(region).findByRole('button', { name: 'Edit A-001' }))
    await userEvent.type(within(region).getByLabelText('Description of A-001'), ' now')
    await userEvent.keyboard('{Escape}')
    expect(within(region).queryByLabelText('Description of A-001')).toBeNull()
    expect(backend.actions[0]!.version).toBe(1)

    await userEvent.click(within(region).getByRole('button', { name: 'Edit A-001' }))
    await userEvent.type(within(region).getByLabelText('Description of A-001'), ' now')
    await userEvent.click(within(region).getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(backend.actions[0]).toMatchObject({ description: 'Draft plan now' }))
  })

  it('closes and reopens an Action', async () => {
    const { backend } = await meeting([
      action({ id: 'x1', reference: 'A-001', description: 'Draft plan' }),
    ])
    const region = await panel()
    await userEvent.click(await within(region).findByRole('button', { name: 'Close A-001' }))
    await waitFor(async () => expect(await openActions()).toHaveLength(0))
    expect(backend.actions[0]).toMatchObject({ closed: true, version: 2 })
    await userEvent.click(within(region).getByText('Closed (1)'))
    await userEvent.click(within(region).getByRole('button', { name: 'Reopen A-001' }))
    await waitFor(async () => expect(await openActions()).toHaveLength(1))
  })

  it('on a conflicting save keeps the unsaved text and shows the new value', async () => {
    const { backend } = await meeting([
      action({ id: 'x1', reference: 'A-001', description: 'Draft plan' }),
    ])
    const region = await panel()
    await userEvent.click(await within(region).findByRole('button', { name: 'Edit A-001' }))
    // Someone else edits the Action after it was loaded.
    Object.assign(backend.actions[0]!, { description: 'Their plan', version: 2 })
    const text = within(region).getByLabelText<HTMLTextAreaElement>('Description of A-001')
    await userEvent.clear(text)
    await userEvent.type(text, 'My plan')
    await userEvent.click(within(region).getByRole('button', { name: 'Save' }))

    expect((await within(region).findByRole('alert')).textContent).toContain('Precondition Failed')
    expect((await within(region).findByRole('status')).textContent).toContain('Their plan')
    expect(within(region).getByLabelText<HTMLTextAreaElement>('Description of A-001').value).toBe(
      'My plan',
    )
    await userEvent.click(within(region).getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(backend.actions[0]).toMatchObject({ description: 'My plan' }))
  })

  it('unassigns open Actions when their assignee is removed', async () => {
    const { backend } = await meeting([
      action({
        id: 'x1',
        reference: 'A-001',
        description: 'Draft plan',
        assignee: { id: 'b', display_name: 'Bob' },
      }),
    ])
    await userEvent.click(await screen.findByRole('button', { name: 'Members' }))
    await userEvent.click(await screen.findByRole('button', { name: 'Remove Bob' }))
    await waitFor(() => expect(backend.actions[0]!.assignee).toBeNull())
    await waitFor(async () =>
      expect((await openActions())[0]!.querySelector('[data-unassigned]')).not.toBeNull(),
    )
  })

  describe('sort and filter', () => {
    const list = () => [
      action({
        id: 'x1',
        reference: 'A-001',
        description: 'Late',
        assignee: { id: 'b', display_name: 'Bob' },
        due_date: '2030-09-01',
      }),
      action({ id: 'x2', reference: 'A-002', description: 'Floating' }),
      action({
        id: 'x3',
        reference: 'A-003',
        description: 'Soon',
        assignee: { id: 'a', display_name: 'Ada' },
        due_date: '2030-01-01',
      }),
      action({ id: 'x4', reference: 'A-004', description: 'Done', closed: true }),
    ]
    const refs = async () =>
      (await openActions()).map((li) => li.querySelector('.font-mono')!.textContent)

    it('sorts by Reference by default, and by due date or assignee on request', async () => {
      await meeting(list())
      await waitFor(async () => expect(await refs()).toEqual(['A-001', 'A-002', 'A-003']))
      const region = await panel()
      await userEvent.selectOptions(within(region).getByLabelText('Sort Actions'), 'due')
      expect(await refs()).toEqual(['A-003', 'A-001', 'A-002'])
      await userEvent.selectOptions(within(region).getByLabelText('Sort Actions'), 'assignee')
      expect(await refs()).toEqual(['A-003', 'A-001', 'A-002'])
    })

    it('filters to unassigned Actions with one click, or by assignee', async () => {
      await meeting(list())
      await waitFor(async () => expect(await openActions()).toHaveLength(3))
      const region = await panel()
      const quick = within(region).getByRole('button', { name: 'Unassigned only' })
      await userEvent.click(quick)
      expect(quick.getAttribute('aria-pressed')).toBe('true')
      expect(await refs()).toEqual(['A-002'])
      await userEvent.click(quick)
      expect(await refs()).toHaveLength(3)

      const assignee = within(region).getByLabelText('Filter Actions by assignee')
      await within(assignee).findByRole('option', { name: 'Bob' })
      await userEvent.selectOptions(assignee, 'b')
      expect(await refs()).toEqual(['A-001'])
    })

    it('filters by state and by due date', async () => {
      await meeting(list())
      await waitFor(async () => expect(await openActions()).toHaveLength(3))
      const region = await panel()
      await userEvent.selectOptions(
        within(region).getByLabelText('Filter Actions by state'),
        'closed',
      )
      expect(await openActions()).toHaveLength(0)
      expect(within(region).getByText('Closed (1)').closest('details')!.open).toBe(true)
      await userEvent.selectOptions(
        within(region).getByLabelText('Filter Actions by state'),
        'open',
      )
      expect(within(region).queryByText('Closed (1)')).toBeNull()

      await userEvent.type(within(region).getByLabelText('Due by'), '2030-06-30')
      expect(await refs()).toEqual(['A-003'])
    })
  })

  it('is read-only in an archived Project', async () => {
    await meeting([action({ id: 'x1', reference: 'A-001', description: 'Draft plan' })], {
      archived: true,
    })
    const region = await panel()
    await waitFor(async () => expect(await openActions()).toHaveLength(1))
    expect(within(region).queryByRole('button', { name: 'Add Action' })).toBeNull()
    expect(within(region).queryByRole('button', { name: 'Close A-001' })).toBeNull()
    expect(within(region).getByLabelText<HTMLSelectElement>('Assignee of A-001').disabled).toBe(
      true,
    )
  })
})
