import { afterEach, describe, expect, it, vi } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { MeetingLock, Project, Risk } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const bob = { id: 'b', version: 1, display_name: 'Bob' }

const project = {
  id: 'p1',
  name: 'Website',
  project_code: 'WEBS',
  description: null,
  archived: false,
  project_leader: { id: 'a', display_name: 'Ada' },
  health_rating: null,
  version: 1,
} as Project

const risk = {
  id: 'r1',
  reference: 'R-001',
  description: 'Vendor late',
  project_id: 'p1',
  project_code: 'WEBS',
  likelihood: 3,
  impact: 3,
  score: 9,
  closed: false,
  version: 1,
} as Risk

// 09:05 local time, so the banner's HH:MM is known whatever the time zone.
const adasMeeting: MeetingLock = {
  leader: { id: 'a', display_name: 'Ada' },
  acquired_at: new Date(2026, 9, 21, 9, 5).toISOString(),
  live: true,
  held_by_you: false,
}

/** Bob, a Member but not the leader, views the Project. */
async function renderAsBob(lock: MeetingLock | null) {
  const view = await renderApp('/projects/p1', [ada, bob], 'b', [project], [risk], [], [], [], lock)
  view.backend.members.get('p1')!.add('b')
  await screen.findByRole('heading', { name: /Website/ })
  return view
}

const banner = () => screen.queryByRole('status', { name: 'Meeting in progress' })
const button = (name: string | RegExp) => screen.getByRole('button', { name })
// `:disabled` also covers a control disabled by its fieldset.
const disabled = (element: HTMLElement) => element.matches(':disabled')

describe('Meeting in progress banner', () => {
  afterEach(() => vi.useRealTimers())

  it('shows who leads the Meeting and disables editing, but not Member management', async () => {
    await renderAsBob(adasMeeting)
    await waitFor(() => expect(banner()).toBeTruthy())
    expect(banner()!.textContent).toMatch(/Meeting in progress, led by Ada, since 09:05/)
    await screen.findByText(/Vendor late/)
    expect(disabled(button('Edit Health Rating'))).toBe(true)
    expect(disabled(button('Add Risk'))).toBe(true)
    expect(disabled(button('Add Action'))).toBe(true)
    expect(disabled(button('Add Blocker'))).toBe(true)

    await userEvent.click(button('Project menu'))
    const menu = screen.getByRole('dialog')
    expect(disabled(within(menu).getByRole('button', { name: 'Manage Members' }))).toBe(false)
  })

  it('shows no banner to the session holding the lock', async () => {
    await renderAsBob({ ...adasMeeting, held_by_you: true })
    await screen.findByText(/Vendor late/)
    await new Promise((r) => setTimeout(r, 50))
    expect(banner()).toBeNull()
    expect(disabled(button('Add Risk'))).toBe(false)
  })

  it('shows no banner for a stale lock', async () => {
    await renderAsBob({ ...adasMeeting, live: false })
    await screen.findByText(/Vendor late/)
    await new Promise((r) => setTimeout(r, 50))
    expect(banner()).toBeNull()
  })

  it('clears the banner, re-enables editing and refetches items when the lock is released', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const { backend } = await renderAsBob(adasMeeting)
    await waitFor(() => expect(banner()).toBeTruthy())

    // Ada's Meeting changed things while Bob watched.
    backend.risks[0]!.description = 'Vendor very late'
    backend.setLock(null)
    await vi.advanceTimersByTimeAsync(30_000)

    await waitFor(() => expect(banner()).toBeNull())
    expect(disabled(button('Add Risk'))).toBe(false)
    expect(await screen.findByText(/Vendor very late/)).toBeTruthy()
  })

  it('shows the banner straight away when a write is refused by a lock', async () => {
    const { backend } = await renderAsBob(null)
    await screen.findByText(/Vendor late/)
    // Ada starts a Meeting before Bob's next poll.
    backend.setLock(adasMeeting)

    await userEvent.click(button('Edit Health Rating'))
    await userEvent.click(screen.getByRole('radio', { name: 'Green' }))
    await userEvent.click(button('Save'))

    await waitFor(() => expect(banner()).toBeTruthy())
    expect(screen.queryByText(/Couldn't save/)).toBeNull()
  })
})
