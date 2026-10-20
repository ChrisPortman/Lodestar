import { describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Project, Risk } from '../test/server'

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

function risk(fields: Partial<Risk> & Pick<Risk, 'id' | 'reference' | 'description'>) {
  const likelihood = fields.likelihood ?? 3
  const impact = fields.impact ?? 3
  return {
    project_id: 'p1',
    project_code: 'WEBS',
    likelihood,
    impact,
    score: likelihood * impact,
    closed: false,
    version: 1,
    ...fields,
  } as Risk
}

async function panel() {
  return screen.findByRole('region', { name: 'Risks' })
}

async function openRisks() {
  const list = within(await panel()).getByRole('list', { name: 'Open Risks' })
  return within(list).queryAllByRole('listitem')
}

describe('Risks panel', () => {
  it('lists open Risks with a score coloured by severity, and closed ones collapsed', async () => {
    await renderApp(
      '/projects/p1',
      [ada],
      'a',
      [project()],
      [
        risk({
          id: 'r1',
          reference: 'R-001',
          description: 'Vendor late\nAgain',
          likelihood: 5,
          impact: 4,
        }),
        risk({ id: 'r2', reference: 'R-002', description: 'Minor', likelihood: 1, impact: 2 }),
        risk({ id: 'r3', reference: 'R-003', description: 'Old one', closed: true }),
      ],
    )
    await waitFor(async () => expect(await openRisks()).toHaveLength(2))
    const [first, second] = await openRisks()
    expect(first!.textContent).toContain('R-001')
    expect(within(first!).getByText(/Vendor late/).textContent).toBe('Vendor late\nAgain')
    const highScore = within(first!).getByLabelText('Score')
    expect(highScore.textContent).toContain('20')
    expect(highScore.dataset.severity).toBe('high')
    expect(within(second!).getByLabelText('Score').dataset.severity).toBe('low')
    expect(first!.dataset.accent).toBe('red')
    expect(second!.dataset.accent).toBeUndefined()
    expect(within(first!).getByText('Likelihood')).toBeTruthy()
    expect(within(first!).getByText('Impact')).toBeTruthy()

    const closed = within(await panel()).getByText('Closed (1)')
    expect(closed.closest('details')!.open).toBe(false)
    expect(within(await panel()).getByText('Old one')).toBeTruthy()
  })

  it('adds a Risk', async () => {
    const { backend } = await renderApp('/projects/p1', [ada], 'a', [project()])
    const region = await panel()
    await userEvent.click(within(region).getByRole('button', { name: 'Add Risk' }))
    await userEvent.type(within(region).getByLabelText('Description'), 'Budget cut')
    await userEvent.selectOptions(within(region).getByLabelText('Likelihood'), '4')
    await userEvent.selectOptions(within(region).getByLabelText('Impact'), '5')
    await userEvent.click(within(region).getByRole('button', { name: 'Add' }))

    await waitFor(async () => expect(await openRisks()).toHaveLength(1))
    expect(backend.risks[0]).toMatchObject({
      reference: 'R-001',
      description: 'Budget cut',
      likelihood: 4,
      impact: 5,
      score: 20,
    })
    expect((await openRisks())[0]!.textContent).toContain('20')
  })

  it('saves likelihood and impact as soon as they change', async () => {
    const { backend } = await renderApp(
      '/projects/p1',
      [ada],
      'a',
      [project()],
      [
        risk({
          id: 'r1',
          reference: 'R-001',
          description: 'Vendor late',
          likelihood: 1,
          impact: 1,
        }),
      ],
    )
    const region = await panel()
    await userEvent.selectOptions(await within(region).findByLabelText('Likelihood of R-001'), '5')
    await waitFor(() => expect(backend.risks[0]).toMatchObject({ likelihood: 5, version: 2 }))
    await waitFor(() => expect(within(region).getByLabelText('Score').textContent).toContain('5'))
    await userEvent.selectOptions(within(region).getByLabelText('Impact of R-001'), '3')
    await waitFor(() =>
      expect(backend.risks[0]).toMatchObject({ impact: 3, score: 15, version: 3 }),
    )
    await waitFor(() =>
      expect(within(region).getByLabelText('Score').dataset.severity).toBe('high'),
    )
  })

  it('edits a description with an explicit Save', async () => {
    const { backend } = await renderApp(
      '/projects/p1',
      [ada],
      'a',
      [project()],
      [risk({ id: 'r1', reference: 'R-001', description: 'Vendor late' })],
    )
    const region = await panel()
    await userEvent.click(await within(region).findByRole('button', { name: 'Edit R-001' }))
    const text = within(region).getByLabelText('Description of R-001')
    await userEvent.clear(text)
    await userEvent.type(text, 'Vendor very late')
    await userEvent.click(within(region).getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(backend.risks[0]).toMatchObject({ description: 'Vendor very late' }))
    expect(await within(region).findByText('Vendor very late')).toBeTruthy()
  })

  it('closes and reopens a Risk', async () => {
    const { backend } = await renderApp(
      '/projects/p1',
      [ada],
      'a',
      [project()],
      [risk({ id: 'r1', reference: 'R-001', description: 'Vendor late' })],
    )
    const region = await panel()
    await userEvent.click(await within(region).findByRole('button', { name: 'Close R-001' }))
    await waitFor(async () => expect(await openRisks()).toHaveLength(0))
    expect(backend.risks[0]).toMatchObject({ closed: true, version: 2 })

    await userEvent.click(within(region).getByText('Closed (1)'))
    await userEvent.click(within(region).getByRole('button', { name: 'Reopen R-001' }))
    await waitFor(async () => expect(await openRisks()).toHaveLength(1))
    expect(backend.risks[0]).toMatchObject({ closed: false, version: 3 })
  })

  it('reports a change made elsewhere and reloads', async () => {
    const { backend } = await renderApp(
      '/projects/p1',
      [ada],
      'a',
      [project()],
      [risk({ id: 'r1', reference: 'R-001', description: 'Vendor late', likelihood: 1 })],
    )
    const region = await panel()
    await within(region).findByLabelText('Likelihood of R-001')
    // Someone else edits the Risk after it was loaded.
    Object.assign(backend.risks[0]!, { likelihood: 2, score: 6, version: 2 })
    await userEvent.selectOptions(within(region).getByLabelText('Likelihood of R-001'), '5')
    expect((await within(region).findByRole('alert')).textContent).toContain('Precondition Failed')
    await waitFor(() =>
      expect(within(region).getByLabelText<HTMLSelectElement>('Likelihood of R-001').value).toBe(
        '2',
      ),
    )
  })

  it('sorts by Reference by default or by score, and filters by state', async () => {
    await renderApp(
      '/projects/p1',
      [ada],
      'a',
      [project()],
      [
        risk({ id: 'r1', reference: 'R-001', description: 'Low', likelihood: 1, impact: 2 }),
        risk({ id: 'r2', reference: 'R-002', description: 'High', likelihood: 5, impact: 5 }),
        risk({ id: 'r3', reference: 'R-003', description: 'Gone', closed: true }),
      ],
    )
    const refs = async () =>
      (await openRisks()).map((li) => li.querySelector('.font-mono')!.textContent)
    await waitFor(async () => expect(await refs()).toEqual(['R-001', 'R-002']))
    const region = await panel()
    await userEvent.selectOptions(within(region).getByLabelText('Sort Risks'), 'score')
    expect(await refs()).toEqual(['R-002', 'R-001'])
    expect(within(region).queryByLabelText('Filter Risks by assignee')).toBeNull()
    await userEvent.selectOptions(within(region).getByLabelText('Filter Risks by state'), 'closed')
    expect(await openRisks()).toHaveLength(0)
    expect(within(region).getByText('Closed (1)').closest('details')!.open).toBe(true)
  })

  it('is read-only in an archived Project', async () => {
    await renderApp(
      '/projects/p1',
      [ada],
      'a',
      [project({ archived: true })],
      [risk({ id: 'r1', reference: 'R-001', description: 'Vendor late' })],
    )
    const region = await panel()
    await waitFor(async () => expect(await openRisks()).toHaveLength(1))
    expect(within(region).queryByRole('button', { name: 'Add Risk' })).toBeNull()
    expect(within(region).queryByRole('button', { name: 'Close R-001' })).toBeNull()
    expect(within(region).getByLabelText<HTMLSelectElement>('Likelihood of R-001').disabled).toBe(
      true,
    )
  })
})
