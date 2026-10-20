import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Comment, Project, Risk } from '../test/server'

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

const vendorRisk = {
  id: 'r1',
  project_id: 'p1',
  project_code: 'WEBS',
  reference: 'R-001',
  description: 'Vendor late',
  likelihood: 3,
  impact: 3,
  score: 9,
  closed: false,
  version: 1,
} as Risk

function comment(fields: Partial<Comment> & Pick<Comment, 'id' | 'body'>) {
  return {
    project_id: 'p1',
    target_type: 'project',
    target_id: 'p1',
    author: { id: 'b', display_name: 'Bob' },
    created_at: '2026-10-01T09:00:00Z',
    edited_at: null,
    deleted: false,
    version: 1,
    ...fields,
  } as Comment
}

/** The Project-level Comments disclosure. */
async function projectComments() {
  const region = await screen.findByRole('region', { name: 'Project Comments' })
  return region.querySelector('details')!
}

function render(comments: Comment[], fields: { project?: Partial<Project>; risks?: Risk[] } = {}) {
  return renderApp(
    '/projects/p1',
    [ada, bob],
    'a',
    [project(fields.project)],
    fields.risks ?? [],
    [],
    [],
    comments,
  )
}

beforeEach(() => localStorage.clear())
afterEach(() => localStorage.clear())

describe('Comments', () => {
  it('lists a Project’s Comments collapsed by default, oldest first', async () => {
    await render([
      comment({ id: 'c1', body: 'First' }),
      comment({ id: 'c2', body: 'Second', created_at: '2026-10-02T09:00:00Z' }),
    ])
    const details = await projectComments()
    await waitFor(() => expect(details.querySelector('summary')!.textContent).toContain('(2)'))
    expect(details.open).toBe(false)
    const items = within(details).getAllByRole('listitem')
    expect(items.map((li) => within(li).getByTestId('comment-body').textContent)).toEqual([
      'First',
      'Second',
    ])
    expect(items[0]!.textContent).toContain('Bob')
  })
  it('shows a Risk’s Comments in its row, badged when new since the last view', async () => {
    localStorage.setItem('lastViewed[p1]', '2026-10-05T00:00:00Z')
    await render(
      [
        comment({ id: 'c1', body: 'Old', target_type: 'risk', target_id: 'r1' }),
        comment({
          id: 'c2',
          body: 'Fresh',
          target_type: 'risk',
          target_id: 'r1',
          created_at: '2026-10-06T00:00:00Z',
        }),
      ],
      { risks: [vendorRisk] },
    )
    const list = await screen.findByRole('list', { name: 'Comments on R-001' })
    const details = list.closest('details')!
    await waitFor(() => expect(details.querySelector('summary')!.textContent).toContain('• new'))
    const [old, fresh] = within(list).getAllByRole('listitem')
    expect(old!.dataset.new).toBeUndefined()
    expect(fresh!.dataset.new).toBe('true')
  })

  it('records this visit as the last view', async () => {
    localStorage.setItem('lastViewed[p1]', '2026-10-05T00:00:00Z')
    await render([])
    await projectComments()
    expect(Date.parse(localStorage.getItem('lastViewed[p1]')!)).toBeGreaterThan(
      Date.parse('2026-10-19T00:00:00Z'),
    )
  })

  it.each([
    [
      'there is no last view',
      null,
      comment({ id: 'c1', body: 'x', created_at: '2026-10-09T00:00:00Z' }),
    ],
    [
      'the Acting Person wrote it',
      '2026-10-05T00:00:00Z',
      comment({
        id: 'c1',
        body: 'x',
        created_at: '2026-10-09T00:00:00Z',
        author: { id: 'a', display_name: 'Ada' },
      }),
    ],
    [
      'it was only edited since',
      '2026-10-05T00:00:00Z',
      comment({ id: 'c1', body: 'x', edited_at: '2026-10-09T00:00:00Z' }),
    ],
    [
      'it is deleted',
      '2026-10-05T00:00:00Z',
      comment({ id: 'c1', body: '', deleted: true, created_at: '2026-10-09T00:00:00Z' }),
    ],
  ])('marks nothing new when %s', async (_, lastViewed, c) => {
    if (lastViewed) localStorage.setItem('lastViewed[p1]', lastViewed)
    await render([c])
    const details = await projectComments()
    await waitFor(() => expect(details.querySelector('summary')!.textContent).toContain('(1)'))
    expect(details.querySelector('summary')!.textContent).not.toContain('new')
    expect(details.querySelector('[data-new]')).toBeNull()
  })

  it('renders bodies as plain text, deleted ones as a placeholder, and edits labelled', async () => {
    await render([
      comment({ id: 'c1', body: '**bold** <b>tag</b>\nline two' }),
      comment({ id: 'c2', body: '', deleted: true }),
      comment({ id: 'c3', body: 'Changed', edited_at: '2026-10-03T09:00:00Z' }),
    ])
    const details = await projectComments()
    await waitFor(() => expect(within(details).getAllByRole('listitem')).toHaveLength(3))
    const [plain, deleted, edited] = within(details).getAllByRole('listitem')
    const body = within(plain!).getByTestId('comment-body')
    expect(body.textContent!.trim()).toBe('**bold** <b>tag</b>\nline two')
    expect(body.querySelector('b, strong')).toBeNull()
    expect(within(deleted!).getByText('Comment deleted by author')).toBeTruthy()
    expect(within(deleted!).queryByTestId('comment-body')).toBeNull()
    expect(within(edited!).getByText(/edited:/)).toBeTruthy()
    expect(within(plain!).queryByText(/edited:/)).toBeNull()
  })

  it('offers Edit and Delete only on the Acting Person’s own Comments', async () => {
    await render([
      comment({ id: 'c1', body: 'Mine', author: { id: 'a', display_name: 'Ada' } }),
      comment({ id: 'c2', body: 'Theirs' }),
      comment({ id: 'c3', body: '', deleted: true, author: { id: 'a', display_name: 'Ada' } }),
    ])
    const details = await projectComments()
    await waitFor(() => expect(within(details).getAllByRole('listitem')).toHaveLength(3))
    const [mine, theirs, gone] = within(details).getAllByRole('listitem')
    expect(within(mine!).getByRole('button', { name: 'Edit' })).toBeTruthy()
    expect(within(mine!).getByRole('button', { name: 'Delete' })).toBeTruthy()
    for (const li of [theirs!, gone!]) {
      expect(within(li).queryByRole('button', { name: 'Edit' })).toBeNull()
      expect(within(li).queryByRole('button', { name: 'Delete' })).toBeNull()
    }
  })

  it('adds, edits and deletes (after confirming) a Comment', async () => {
    const { backend } = await render([])
    const details = await projectComments()
    await userEvent.click(details.querySelector('summary')!)
    await userEvent.type(
      within(details).getByRole('textbox', { name: 'Add a Comment on the Project' }),
      'Hello',
    )
    await userEvent.click(within(details).getByRole('button', { name: 'Comment' }))
    await within(details).findByText('Hello')

    await userEvent.click(within(details).getByRole('button', { name: 'Edit' }))
    const editor = within(details).getByRole('textbox', { name: 'Edit Comment' })
    await userEvent.clear(editor)
    await userEvent.type(editor, 'Hi')
    await userEvent.click(within(details).getByRole('button', { name: 'Save' }))
    await within(details).findByText(/edited:/)
    expect(backend.comments[0]!.body).toBe('Hi')

    await userEvent.click(await within(details).findByRole('button', { name: 'Delete' }))
    expect(backend.comments[0]!.deleted).toBe(false)
    await userEvent.click(within(details).getByRole('button', { name: 'Yes, delete' }))
    await within(details).findByText('Comment deleted by author')
    expect(backend.comments[0]!.deleted).toBe(true)
  })

  it('refreshes after a 412 so the latest text shows', async () => {
    const { backend } = await render([
      comment({ id: 'c1', body: 'Mine', author: { id: 'a', display_name: 'Ada' } }),
    ])
    const details = await projectComments()
    await within(details).findByText('Mine')
    backend.comments[0]!.body = 'Changed elsewhere'
    backend.comments[0]!.version = 2
    await userEvent.click(within(details).getByRole('button', { name: 'Delete' }))
    await userEvent.click(within(details).getByRole('button', { name: 'Yes, delete' }))
    await within(details).findByRole('alert')
    await within(details).findByText('Changed elsewhere')
  })

  it.each([
    ['the Project is archived', { archived: true }, 'a'],
    ['the Acting Person isn’t a Member', {}, 'b'],
  ])('hides add, edit and delete when %s', async (_, fields: Partial<Project>, acting) => {
    await renderApp(
      '/projects/p1',
      [ada, bob],
      acting,
      [project(fields)],
      [],
      [],
      [],
      [comment({ id: 'c1', body: 'Mine', author: { id: acting, display_name: 'X' } })],
    )
    const details = await projectComments()
    await within(details).findByText('Mine')
    // Membership loads separately; give it time to settle before asserting absence.
    await screen.findByRole('heading', { name: /Website/ })
    await new Promise((r) => setTimeout(r, 50))
    expect(within(details).queryByRole('textbox')).toBeNull()
    expect(within(details).queryByRole('button', { name: 'Edit' })).toBeNull()
  })
})
