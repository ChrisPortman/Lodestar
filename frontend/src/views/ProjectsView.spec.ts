import { describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { Project } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const leader = { id: 'a', display_name: 'Ada' }

function project(fields: Partial<Project> & Pick<Project, 'id' | 'name' | 'project_code'>) {
  return {
    description: null,
    archived: false,
    project_leader: leader,
    health_rating: null,
    version: 1,
    ...fields,
  } as Project
}

const web = project({ id: 'p1', name: 'Website', project_code: 'WEBS', description: 'New site' })
const old = project({ id: 'p2', name: 'Old Thing', project_code: 'OLDT', archived: true })

async function projectNames() {
  const list = await screen.findByRole('list', { name: 'Projects' })
  return within(list)
    .queryAllByRole('listitem')
    .map((li) => li.textContent ?? '')
}

describe('Projects page', () => {
  it('lists Projects, hiding archived ones unless asked', async () => {
    await renderApp('/projects', [ada], 'a', [web, old])
    await waitFor(async () => expect(await projectNames()).toHaveLength(1))
    expect((await projectNames())[0]).toContain('WEBS')

    await userEvent.click(screen.getByLabelText('Show archived'))
    await waitFor(async () => expect(await projectNames()).toHaveLength(2))
    expect((await projectNames())[1]).toContain('Archived')
  })

  it('accents Red-rated and leaderless Project cards, with a word as well as a colour', async () => {
    const red = project({
      id: 'p3',
      name: 'Red One',
      project_code: 'REDO',
      health_rating: { rating: 'red', comment: null },
    })
    const orphan = project({ id: 'p4', name: 'Orphan', project_code: 'ORPH', project_leader: null })
    await renderApp('/projects', [ada], 'a', [web, red, orphan])
    const list = await screen.findByRole('list', { name: 'Projects' })
    await waitFor(() => expect(within(list).getAllByRole('listitem')).toHaveLength(3))
    const [plain, redCard, orphanCard] = within(list).getAllByRole('listitem')
    expect(plain!.dataset.accent).toBeUndefined()
    expect(redCard!.dataset.accent).toBe('red')
    expect(redCard!.textContent).toContain('▲ Red')
    expect(orphanCard!.dataset.accent).toBe('amber')
    expect(orphanCard!.textContent).toMatch(/Leader\s*none/i)
  })

  it('creates a Project with the suggested code, which can be edited', async () => {
    const { backend } = await renderApp('/projects', [ada], 'a', [web])
    await userEvent.click(await screen.findByRole('button', { name: 'New Project' }))
    const dialog = screen.getByRole('dialog', { name: 'New Project' })
    await userEvent.type(within(dialog).getByLabelText('Name'), 'Website')
    const code = within(dialog).getByLabelText<HTMLInputElement>('Project Code')
    await waitFor(() => expect(code.value).toBe('WEBS1'))

    await userEvent.clear(code)
    await userEvent.type(code, 'SITE2')
    await userEvent.type(within(dialog).getByLabelText('Description'), 'Second go')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Create' }))

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(backend.projects.at(-1)).toMatchObject({
      name: 'Website',
      project_code: 'SITE2',
      description: 'Second go',
    })
    await waitFor(async () => expect((await projectNames()).join()).toContain('SITE2'))
  })

  it('reports a code that is already in use', async () => {
    await renderApp('/projects', [ada], 'a', [web])
    await userEvent.click(await screen.findByRole('button', { name: 'New Project' }))
    const dialog = screen.getByRole('dialog', { name: 'New Project' })
    await userEvent.type(within(dialog).getByLabelText('Name'), 'Other')
    const code = within(dialog).getByLabelText<HTMLInputElement>('Project Code')
    await waitFor(() => expect(code.value).toBe('OTHE'))
    await userEvent.clear(code)
    await userEvent.type(code, 'WEBS')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Create' }))
    expect((await within(dialog).findByRole('alert')).textContent).toContain('Conflict')
  })

  it('renames and describes a Project', async () => {
    const { backend } = await renderApp('/projects', [ada], 'a', [web])
    await userEvent.click(await screen.findByRole('button', { name: 'Edit Website' }))
    const name = screen.getByLabelText('Name for Website')
    await userEvent.clear(name)
    await userEvent.type(name, 'Web Rebuild')
    await userEvent.clear(screen.getByLabelText('Description for Website'))
    await userEvent.click(screen.getByRole('button', { name: 'Save' }))
    await screen.findByRole('button', { name: 'Edit Web Rebuild' })
    expect(backend.projects[0]).toMatchObject({
      name: 'Web Rebuild',
      description: null,
      project_code: 'WEBS',
      version: 2,
    })
  })

  it('archives and unarchives a Project', async () => {
    const { backend } = await renderApp('/projects', [ada], 'a', [web])
    await userEvent.click(await screen.findByLabelText('Show archived'))
    await userEvent.click(await screen.findByRole('button', { name: 'Archive Website' }))
    await screen.findByRole('button', { name: 'Unarchive Website' })
    expect(backend.projects[0]).toMatchObject({ archived: true, version: 2 })
    expect(screen.queryByRole('button', { name: 'Edit Website' })).toBeNull()

    await userEvent.click(screen.getByRole('button', { name: 'Unarchive Website' }))
    await screen.findByRole('button', { name: 'Archive Website' })
    expect(backend.projects[0]).toMatchObject({ archived: false, version: 3 })
  })
})
