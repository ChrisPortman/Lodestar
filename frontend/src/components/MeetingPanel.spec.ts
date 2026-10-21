import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { MeetingLock, Project } from '../test/server'

const ada = { id: 'a', version: 1, display_name: 'Ada' }
const bob = { id: 'b', version: 1, display_name: 'Bob' }
const cy = { id: 'c', version: 1, display_name: 'Cy' }
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

beforeEach(() => localStorage.clear())
afterEach(() => vi.useRealTimers())

async function renderWithMembers(lock: MeetingLock | null = null) {
  const view = await renderApp('/projects/p1', [ada, bob, cy], 'a', [project], [], [], [], [], lock)
  view.backend.members.get('p1')!.add('b').add('c')
  return view
}

async function startMeeting() {
  await userEvent.click(await screen.findByRole('button', { name: 'Project menu' }))
  await userEvent.click(screen.getByRole('button', { name: 'Meetings' }))
  await userEvent.click(screen.getByRole('button', { name: 'Start a Meeting now' }))
  return screen.findByRole('dialog', { name: 'Meeting' })
}

describe('the Meeting panel (#85)', () => {
  it('opens on starting a Meeting, closing the Project menu', async () => {
    await renderWithMembers()
    const panel = await startMeeting()
    expect(within(panel).getByRole('timer')).toBeTruthy()
    expect(screen.queryByRole('dialog', { name: 'Meetings' })).toBeNull()
  })

  it('closes to a strip that counts Comments and reopens it; the menu closes it', async () => {
    await renderWithMembers()
    const panel = await startMeeting()
    await userEvent.type(within(panel).getByLabelText('Meeting Comment'), 'Venue booked{Enter}')
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: 'Meeting' })).toBeNull()
    const strip = screen.getByRole('button', { name: /● Meeting .* · 1 Comments/ })
    await userEvent.click(strip)
    expect(screen.getByRole('dialog', { name: 'Meeting' })).toBeTruthy()

    await userEvent.click(screen.getByRole('button', { name: 'Project menu' }))
    expect(screen.queryByRole('dialog', { name: 'Meeting' })).toBeNull()
    expect(screen.getByRole('dialog', { name: 'Project menu' })).toBeTruthy()
    await userEvent.click(screen.getByRole('button', { name: 'Meetings' }))
    await userEvent.click(screen.getByRole('button', { name: 'Open the Meeting' }))
    expect(screen.queryByRole('dialog', { name: 'Meetings' })).toBeNull()
    expect(screen.getByRole('dialog', { name: 'Meeting' })).toBeTruthy()
  })

  it('always marks the leader Here, and toggles other Members', async () => {
    await renderWithMembers()
    const panel = await startMeeting()
    const adaBox = await within(panel).findByRole('checkbox', { name: /Ada/ })
    expect((adaBox as HTMLInputElement).checked).toBe(true)
    expect(adaBox.matches(':disabled')).toBe(true)
    const bobBox = within(panel).getByRole('checkbox', { name: 'Bob' }) as HTMLInputElement
    await userEvent.click(bobBox)
    expect(bobBox.checked).toBe(true)
    await userEvent.click(bobBox)
    expect(bobBox.checked).toBe(false)
  })

  it('ends with Who attended?, creates the Minutes and opens them in the reader', async () => {
    const { backend } = await renderWithMembers()
    const panel = await startMeeting()
    await userEvent.click(await within(panel).findByRole('checkbox', { name: 'Bob' }))
    await userEvent.type(within(panel).getByLabelText('Meeting Comment'), 'Venue booked{Enter}')
    await userEvent.click(within(panel).getByRole('button', { name: 'End Meeting…' }))
    expect(within(panel).getByRole('heading', { name: 'Who attended?' })).toBeTruthy()
    expect(within(panel).getByText(/Ada, Bob\. Mark anyone missing/)).toBeTruthy()
    // A late arrival can still be marked.
    await userEvent.click(within(panel).getByRole('checkbox', { name: 'Cy' }))
    await userEvent.click(within(panel).getByRole('button', { name: 'Create Minutes' }))

    const reader = await screen.findByRole('dialog', { name: /Minutes/ })
    expect(await within(reader).findByText(/Formatted Minutes of meeting-/)).toBeTruthy()
    expect(backend.created).toEqual([
      { project_id: 'p1', attendee_ids: ['a', 'b', 'c'], comments: [{ body: 'Venue booked' }] },
    ])
    expect(screen.queryByRole('dialog', { name: 'Meeting' })).toBeNull()
    expect(screen.queryByRole('button', { name: /● Meeting/ })).toBeNull()
    expect(localStorage.getItem('lodestar.meeting.p1')).toBeNull()
  })

  it('goes Back from Who attended? to the running Meeting', async () => {
    await renderWithMembers()
    const panel = await startMeeting()
    await userEvent.click(within(panel).getByRole('button', { name: 'End Meeting…' }))
    await userEvent.click(within(panel).getByRole('button', { name: 'Back' }))
    expect(within(panel).getByRole('heading', { name: 'Here' })).toBeTruthy()
    expect(within(panel).getByRole('button', { name: 'End Meeting…' })).toBeTruthy()
  })

  it('cancels after confirming, releasing the lock and discarding the Comments', async () => {
    const { backend } = await renderWithMembers()
    const panel = await startMeeting()
    await userEvent.type(within(panel).getByLabelText('Meeting Comment'), 'Draft{Enter}')
    await userEvent.click(within(panel).getByRole('button', { name: 'Cancel' }))
    expect(backend.lock()).not.toBeNull()
    await userEvent.click(within(panel).getByRole('button', { name: 'Yes, cancel it' }))
    await waitFor(() => expect(backend.lock()).toBeNull())
    expect(screen.queryByRole('dialog', { name: 'Meeting' })).toBeNull()
    expect(screen.queryByRole('button', { name: /● Meeting/ })).toBeNull()
    expect(localStorage.getItem('lodestar.meeting.p1')).toBeNull()
    expect(backend.created).toEqual([])
  })

  it('heartbeats the lock every 30 seconds', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const { backend } = await renderWithMembers()
    await startMeeting()
    await vi.advanceTimersByTimeAsync(30_000)
    await waitFor(() => expect(backend.heartbeats()).toBe(1))
    await vi.advanceTimersByTimeAsync(30_000)
    await waitFor(() => expect(backend.heartbeats()).toBe(2))
  })

  it('tells the leader when someone took over, keeping their Comments to copy', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const { backend } = await renderWithMembers()
    const panel = await startMeeting()
    await userEvent.type(within(panel).getByLabelText('Meeting Comment'), 'Keep me{Enter}')
    backend.setLock({
      ...backend.lock()!,
      leader: { id: 'b', display_name: 'Bob' },
      held_by_you: false,
    })
    await vi.advanceTimersByTimeAsync(30_000)
    expect(await within(panel).findByRole('alert')).toHaveProperty(
      'textContent',
      expect.stringMatching(/taken over/),
    )
    expect(within(panel).getByText('Keep me')).toBeTruthy()
  })

  it('tells the leader when creating the Minutes finds the lock taken over', async () => {
    const { backend } = await renderWithMembers()
    const panel = await startMeeting()
    await userEvent.type(within(panel).getByLabelText('Meeting Comment'), 'Keep me{Enter}')
    await userEvent.click(within(panel).getByRole('button', { name: 'End Meeting…' }))
    backend.setLock({ ...backend.lock()!, held_by_you: false })
    await userEvent.click(within(panel).getByRole('button', { name: 'Create Minutes' }))
    expect(await within(panel).findByText(/taken over/)).toBeTruthy()
    expect(within(panel).getByText('Keep me')).toBeTruthy()
  })
})

describe('resuming a Meeting after a reload (#85)', () => {
  const held: MeetingLock = {
    leader: { id: 'a', display_name: 'Ada' },
    acquired_at: new Date(Date.now() - 5 * 60_000).toISOString(),
    live: true,
    held_by_you: true,
  }

  beforeEach(() =>
    localStorage.setItem(
      'lodestar.meeting.p1',
      JSON.stringify({
        startedAt: held.acquired_at,
        attendeeIds: ['b'],
        comments: [{ body: 'Before the crash', at: held.acquired_at }],
      }),
    ),
  )

  it('offers Carry on, which restores attendees and Comments', async () => {
    await renderWithMembers(held)
    const panel = await screen.findByRole('dialog', { name: 'Meeting' })
    await userEvent.click(within(panel).getByRole('button', { name: 'Carry on' }))
    expect(within(panel).getByText('Before the crash')).toBeTruthy()
    const bobBox = await within(panel).findByRole('checkbox', { name: 'Bob' })
    expect((bobBox as HTMLInputElement).checked).toBe(true)
    expect(within(panel).getByRole('timer').textContent).toMatch(/^\s*5:0\d\s*$/)
  })

  it('offers Cancel the Meeting, which releases the lock', async () => {
    const { backend } = await renderWithMembers(held)
    const panel = await screen.findByRole('dialog', { name: 'Meeting' })
    await userEvent.click(within(panel).getByRole('button', { name: 'Cancel the Meeting' }))
    await userEvent.click(within(panel).getByRole('button', { name: 'Yes, cancel it' }))
    await waitFor(() => expect(backend.lock()).toBeNull())
    expect(localStorage.getItem('lodestar.meeting.p1')).toBeNull()
  })
})
