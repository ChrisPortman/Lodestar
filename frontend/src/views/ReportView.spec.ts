import { describe, expect, it } from 'vitest'
import { screen, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Action, Blocker, Project, Risk } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }

function project(fields: Partial<Project> = {}) {
  return {
    id: 'p1',
    name: 'Website',
    project_code: 'WEBS',
    description: 'Rebuild the public site.',
    archived: false,
    project_leader: { id: 'a', display_name: 'Ada' },
    health_rating: { rating: 'amber', comment: 'Vendor is late' },
    version: 1,
    ...fields,
  } as Project
}

const item = { project_id: 'p1', project_code: 'WEBS', version: 1 }

function action(fields: Partial<Action> & Pick<Action, 'id' | 'reference' | 'description'>) {
  return { ...item, assignee: null, due_date: null, closed: false, blocked_by: [], ...fields }
}

function blocker(fields: Partial<Blocker> & Pick<Blocker, 'id' | 'reference' | 'description'>) {
  return { ...item, accountable: null, cleared: false, blocked_actions: [], ...fields }
}

function risk(fields: Partial<Risk> & Pick<Risk, 'id' | 'reference' | 'description'>) {
  return { ...item, likelihood: 1, impact: 1, score: 1, closed: false, ...fields } as Risk
}

async function report(
  fields: Partial<Project> = {},
  items: { actions?: Action[]; blockers?: Blocker[]; risks?: Risk[] } = {},
) {
  return renderApp(
    '/projects/p1/report',
    [ada],
    'a',
    [project(fields)],
    items.risks ?? [],
    items.actions ?? [],
    items.blockers ?? [],
  )
}

/** The body rows of the named table, each as its cell texts. */
async function rows(name: string) {
  const table = await screen.findByRole('table', { name })
  return within(table)
    .getAllByRole('row')
    .slice(1)
    .map((row) =>
      within(row)
        .getAllByRole('cell')
        .map((c) => c.textContent!.trim()),
    )
}

describe('Project on a Page', () => {
  it('heads the page with the Project, its Leader and Health Rating', async () => {
    await report()
    expect(await screen.findByText('WEBS · Project on a Page')).toBeTruthy()
    expect(screen.getByRole('heading', { level: 1 }).textContent!.trim()).toBe('Website')
    expect(screen.getByText('Project Leader: Ada')).toBeTruthy()
    expect(screen.getByText('Amber')).toBeTruthy()
    expect(screen.getByText(/^Generated \d{4}-\d{2}-\d{2}$/)).toBeTruthy()
    expect(screen.getByText('“Vendor is late”')).toBeTruthy()
    expect(screen.getByText('Rebuild the public site.')).toBeTruthy()
  })

  it('says when there is no Leader or Health Rating', async () => {
    await report({ project_leader: null, health_rating: null })
    expect(await screen.findByText('Project Leader: none')).toBeTruthy()
    expect(screen.getByText('Not rated')).toBeTruthy()
  })

  it('lists open Actions with unassigned and overdue ones flagged', async () => {
    await report(
      {},
      {
        actions: [
          action({
            id: 'x1',
            reference: 'WEBS-A-001',
            description: 'Draft plan',
            due_date: '2001-01-01',
            blocked_by: [{ id: 'k1', reference: 'WEBS-B-001' }],
          }),
          action({
            id: 'x2',
            reference: 'WEBS-A-002',
            description: 'Book room',
            assignee: { id: 'a', display_name: 'Ada' },
            due_date: '2999-01-01',
          }),
          action({ id: 'x3', reference: 'WEBS-A-003', description: 'Done', closed: true }),
        ],
        blockers: [
          blocker({
            id: 'k1',
            reference: 'WEBS-B-001',
            description: 'No budget',
            blocked_actions: [{ id: 'x1', reference: 'WEBS-A-001' }],
          }),
        ],
      },
    )
    expect(await rows('Open Actions')).toEqual([
      ['WEBS-A-001', 'Draft plan', 'Unassigned', '2001-01-01', 'WEBS-B-001'],
      ['WEBS-A-002', 'Book room', 'Ada', '2999-01-01', ''],
    ])
    expect(screen.getByText('Unassigned').dataset.unassigned).toBe('true')
    expect(screen.getByText('2001-01-01').dataset.overdue).toBe('true')
    expect(screen.getByText('2999-01-01').dataset.overdue).toBe('false')
  })

  it('lists open Blockers with the Actions they block', async () => {
    await report(
      {},
      {
        blockers: [
          blocker({
            id: 'k1',
            reference: 'WEBS-B-001',
            description: 'No budget',
            accountable: { id: 'a', display_name: 'Ada' },
            blocked_actions: [
              { id: 'x1', reference: 'WEBS-A-001' },
              { id: 'x2', reference: 'WEBS-A-002' },
            ],
          }),
          blocker({ id: 'k2', reference: 'WEBS-B-002', description: 'Gone', cleared: true }),
        ],
      },
    )
    expect(await rows('Open Blockers')).toEqual([
      ['WEBS-B-001', 'No budget', 'Ada', 'WEBS-A-001, WEBS-A-002'],
    ])
  })

  it('lists open Risks highest score first', async () => {
    await report(
      {},
      {
        risks: [
          risk({ id: 'r1', reference: 'WEBS-R-001', description: 'Low', impact: 2, score: 2 }),
          risk({
            id: 'r2',
            reference: 'WEBS-R-002',
            description: 'High',
            likelihood: 4,
            impact: 5,
            score: 20,
          }),
          risk({ id: 'r3', reference: 'WEBS-R-003', description: 'Shut', score: 25, closed: true }),
        ],
      },
    )
    expect(await rows('Open Risks')).toEqual([
      ['WEBS-R-002', 'High', '4', '5', '20'],
      ['WEBS-R-001', 'Low', '1', '2', '2'],
    ])
    expect(screen.getByText('20').dataset.severity).toBe('high')
  })

  it('is linked from the meeting view', async () => {
    await renderApp('/projects/p1', [ada], 'a', [project()])
    await userEvent.click(await screen.findByRole('link', { name: 'Project on a Page' }))
    expect(await screen.findByText('WEBS · Project on a Page')).toBeTruthy()
  })
})
