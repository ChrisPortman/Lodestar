import { describe, expect, it } from 'vitest'
import { screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const bob = { id: 'b', version: 1, display_name: 'Bob' }

describe('Acting Person picker', () => {
  it('picking an existing Person starts a session and goes to Projects', async () => {
    const { router, backend } = await renderApp('/', [ada, bob])
    await userEvent.click(await screen.findByRole('button', { name: 'Bob' }))
    await screen.findByRole('heading', { name: 'Projects' })
    expect(router.currentRoute.value.path).toBe('/projects')
    expect(backend.acting()).toBe('b')
    expect((await screen.findByTestId('acting-person')).textContent).toBe('Bob')
  })

  it('creating a new Person makes them the Acting Person', async () => {
    const { router, backend } = await renderApp('/', [ada])
    await userEvent.type(await screen.findByLabelText("New Person's name"), '  Cleo ')
    await userEvent.click(screen.getByRole('button', { name: "I'm new" }))
    await screen.findByRole('heading', { name: 'Projects' })
    expect(router.currentRoute.value.path).toBe('/projects')
    expect(backend.people.map((p) => p.display_name)).toEqual(['Ada', 'Cleo'])
    expect((await screen.findByTestId('acting-person')).textContent).toBe('Cleo')
  })

  it('skips the picker when a session already exists', async () => {
    const { router } = await renderApp('/', [ada], 'a')
    await screen.findByRole('heading', { name: 'Projects' })
    expect(router.currentRoute.value.path).toBe('/projects')
  })

  it('sends visitors without a session to the picker', async () => {
    const { router } = await renderApp('/people', [ada])
    await screen.findByRole('heading', { name: 'Who are you?' })
    expect(router.currentRoute.value.path).toBe('/')
  })

  it('the header switches the Acting Person', async () => {
    const { backend } = await renderApp('/projects', [ada, bob], 'a')
    expect((await screen.findByTestId('acting-person')).textContent).toBe('Ada')
    await screen.findByRole('option', { name: 'Bob' })
    await userEvent.selectOptions(screen.getByLabelText('Switch Acting Person'), 'Bob')
    await waitFor(() => expect(screen.getByTestId('acting-person').textContent).toBe('Bob'))
    expect(backend.acting()).toBe('b')
  })

  it('the header switches to a new Person (#59)', async () => {
    const { backend } = await renderApp('/projects', [ada], 'a')
    await screen.findByTestId('acting-person')
    await userEvent.selectOptions(screen.getByLabelText('Switch Acting Person'), 'New Person…')
    await userEvent.type(screen.getByLabelText('New Acting Person'), 'Dave{Enter}')
    await waitFor(() => expect(screen.getByTestId('acting-person').textContent).toBe('Dave'))
    expect(backend.acting()).toBe(backend.people[1]!.id)
  })
})
