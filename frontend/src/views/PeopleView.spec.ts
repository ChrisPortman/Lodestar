import { describe, expect, it } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const bob = { id: 'b', version: 3, display_name: 'Bob' }

describe('People page', () => {
  it('lists People', async () => {
    await renderApp('/people', [ada, bob], 'a')
    const list = await screen.findByRole('list', { name: 'People' })
    await within(list).findByText('Bob')
    expect(
      within(list)
        .getAllByRole('listitem')
        .map((li) => li.textContent),
    ).toEqual([expect.stringContaining('Ada'), expect.stringContaining('Bob')])
  })

  it('renames a Person, sending If-Match for their version', async () => {
    const { backend } = await renderApp('/people', [ada, bob], 'a')
    await userEvent.click(await screen.findByRole('button', { name: 'Rename Bob' }))
    const input = screen.getByLabelText('Display name for Bob')
    await userEvent.clear(input)
    await userEvent.type(input, 'Robert')
    await userEvent.click(screen.getByRole('button', { name: 'Save' }))
    await screen.findByRole('button', { name: 'Rename Robert' })
    expect(backend.people[1]).toEqual({ id: 'b', version: 4, display_name: 'Robert' })
  })

  it('renaming the Acting Person updates the header', async () => {
    await renderApp('/people', [ada], 'a')
    await userEvent.click(await screen.findByRole('button', { name: 'Rename Ada' }))
    const input = screen.getByLabelText('Display name for Ada')
    await userEvent.clear(input)
    await userEvent.type(input, 'Ada L{Enter}')
    await waitFor(() => expect(screen.getByTestId('acting-person').textContent).toBe('Ada L'))
  })

  it('reports a rename that lost a race with another edit', async () => {
    const { backend } = await renderApp('/people', [ada, bob], 'a')
    await userEvent.click(await screen.findByRole('button', { name: 'Rename Bob' }))
    backend.people[1].version = 9
    await userEvent.type(screen.getByLabelText('Display name for Bob'), 'by{Enter}')
    expect((await screen.findByRole('alert')).textContent).toContain('Precondition Failed')
  })

  it('creates a Person', async () => {
    const { backend } = await renderApp('/people', [ada], 'a')
    await userEvent.type(await screen.findByLabelText('New Person'), 'Dora')
    await userEvent.click(screen.getByRole('button', { name: 'Add' }))
    await screen.findByRole('button', { name: 'Rename Dora' })
    expect(backend.people.map((p) => p.display_name)).toEqual(['Ada', 'Dora'])
  })
})
