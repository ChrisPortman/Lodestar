import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import type { MeetingLock, MeetingSummary, Project } from '../test/server'

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

const meeting = (id: string, start: Date, minutes: number, attendees: number): MeetingSummary => ({
  id,
  project_id: 'p1',
  leader: { id: 'a', display_name: 'Ada' },
  attendees: [
    { id: 'a', display_name: 'Ada' },
    { id: 'b', display_name: 'Bob' },
    { id: 'c', display_name: 'Cy' },
  ].slice(0, attendees),
  started_at: start.toISOString(),
  ended_at: new Date(start.getTime() + minutes * 60_000).toISOString(),
  version: 1,
})

// Local times, so the rows' clock times are known whatever the time zone.
const older = meeting('m1', new Date(2026, 7, 31, 14, 0), 45, 2)
const newer = meeting('m2', new Date(2026, 9, 21, 9, 0), 75, 3)

async function openMeetings() {
  await userEvent.click(await screen.findByRole('button', { name: 'Project menu' }))
  await userEvent.click(screen.getByRole('button', { name: 'Meetings' }))
  return screen.getByRole('dialog', { name: 'Meetings' })
}

describe('Meetings in the Project menu (#84)', () => {
  it('lets the Project Leader start a Meeting, which acquires the lock', async () => {
    const { backend } = await renderApp('/projects/p1', [ada, bob], 'a', [project])
    const panel = await openMeetings()
    await userEvent.click(within(panel).getByRole('button', { name: 'Start a Meeting now' }))
    await waitFor(() => expect(backend.lock()?.held_by_you).toBe(true))
    // The Meeting panel replaces the menu (#85); back in the menu, it can be reopened.
    expect(await screen.findByRole('dialog', { name: 'Meeting' })).toBeTruthy()
    const again = await openMeetings()
    expect(within(again).getByText(/Your Meeting is running/)).toBeTruthy()
    expect(within(again).queryByRole('button', { name: 'Start a Meeting now' })).toBeNull()
  })

  it('tells a Member that only a Project Leader can start one', async () => {
    const { backend } = await renderApp('/projects/p1', [ada, bob], 'b', [project])
    backend.members.get('p1')!.add('b')
    const panel = await openMeetings()
    expect(within(panel).getByText('Only a Project Leader can start a Meeting.')).toBeTruthy()
    expect(within(panel).queryByRole('button', { name: 'Start a Meeting now' })).toBeNull()
  })

  it("shows everyone who is running another session's live Meeting, and since when", async () => {
    const lock: MeetingLock = {
      leader: { id: 'a', display_name: 'Ada' },
      acquired_at: new Date(2026, 9, 21, 9, 5).toISOString(),
      live: true,
      held_by_you: false,
    }
    await renderApp('/projects/p1', [ada], 'a', [project], [], [], [], [], lock)
    const panel = await openMeetings()
    expect(await within(panel).findByText('Ada is running a Meeting, since 09:05.')).toBeTruthy()
    expect(within(panel).queryByRole('button', { name: 'Start a Meeting now' })).toBeNull()
  })

  it('lists Past Meetings newest first with length, leader and attendee count', async () => {
    const { backend } = await renderApp('/projects/p1', [ada], 'a', [project])
    backend.meetings.push(older, newer)
    const panel = await openMeetings()
    const list = await within(panel).findByRole('list', { name: 'Past Meetings' })
    const rows = await within(list).findAllByRole('button')
    expect(
      rows.map((r) => [...r.children].map((line) => line.textContent!.replace(/\s+/g, ' ').trim())),
    ).toEqual([
      ['Wed 21 Oct 2026, 09:00', '75 min · led by Ada · 3 attendees'],
      ['Mon 31 Aug 2026, 14:00', '45 min · led by Ada · 2 attendees'],
    ])
  })
})

describe('the Minutes reader (#84)', () => {
  let write: ReturnType<typeof vi.fn>
  beforeEach(() => {
    write = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal(
      'ClipboardItem',
      class {
        constructor(readonly items: Record<string, Blob>) {}
      },
    )
    Object.defineProperty(navigator, 'clipboard', { value: { write }, configurable: true })
  })
  afterEach(() => vi.unstubAllGlobals())

  async function openReader() {
    const view = await renderApp('/projects/p1', [ada], 'a', [project])
    view.backend.meetings.push(newer)
    const panel = await openMeetings()
    await userEvent.click(await within(panel).findByRole('button', { name: /21 Oct 2026/ }))
    return { panel, reader: screen.getByRole('dialog', { name: /Minutes/ }) }
  }

  it('opens over the page without closing the menu, and closes alone on Esc', async () => {
    const { reader } = await openReader()
    expect(await within(reader).findByText('Formatted Minutes of m2')).toBeTruthy()
    expect(screen.getByRole('dialog', { name: 'Meetings' })).toBeTruthy()
    within(reader).getByRole('button', { name: 'Close' }).focus()
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: /Minutes/ })).toBeNull()
    expect(screen.getByRole('dialog', { name: 'Meetings' })).toBeTruthy()
  })

  it('renders the Formatted preview from the Minutes document, escaping its text', async () => {
    const { reader } = await openReader()
    const preview = await within(reader).findByRole('article', { name: 'Formatted Minutes' })
    const text = (name: string) => within(preview).getByText(name)
    expect(within(preview).getByRole('heading', { level: 3, name: 'Formatted Minutes of m2' }))
    // 09:00–10:15 UTC is 20:00–21:15 in Sydney (the test zone), named as the browser names it.
    const when = within(preview).getByText(/^Wed 21 Oct 2026, 20:00–21:15 /)
    expect(when.textContent).toMatch(/^Wed 21 Oct 2026, 20:00–21:15 (AEDT|GMT\+11) \(75 min\)$/)
    expect(preview.textContent).toContain('Led by Ada. Attendees: Ada, Bob')
    expect(within(preview).getByRole('heading', { name: 'Actions' })).toBeTruthy()
    expect(within(preview).getByRole('heading', { name: 'New Actions' })).toBeTruthy()
    expect(within(preview).getByRole('heading', { name: 'Updated Actions' })).toBeTruthy()
    expect(preview.textContent).toContain('A-1: <b>Ship</b> (Due 31 Oct)')
    expect(text('Soon')).toBeTruthy()
    expect(text('Closed')).toBeTruthy()
    expect(within(preview).getByRole('heading', { name: 'Meeting Comments' })).toBeTruthy()
    expect(text('Good meeting')).toBeTruthy()
    expect(preview.querySelector('[style]')).toBeNull()
  })

  it('previews the plain text version', async () => {
    const { reader } = await openReader()
    await userEvent.click(within(reader).getByRole('tab', { name: 'Plain text' }))
    expect(await within(reader).findByText(/^Plain Minutes of m2 at UTC660 /)).toBeTruthy()
  })

  it('copies formatted Minutes as HTML with a plain-text fallback', async () => {
    const { reader } = await openReader()
    await userEvent.click(within(reader).getByRole('button', { name: 'Copy formatted' }))
    await waitFor(() => expect(write).toHaveBeenCalledOnce())
    const [item] = write.mock.calls[0]![0] as { items: Record<string, Blob> }[]
    expect(Object.keys(item!.items).sort()).toEqual(['text/html', 'text/plain'])
    // Rendered in the reader's zone at the Meeting's start (AEDT, UTC+11:00).
    expect(await item!.items['text/html']!.text()).toMatch(
      /^<p>Formatted Minutes of m2 at UTC660 (AEDT|GMT\+11)<\/p>$/,
    )
    expect(await item!.items['text/plain']!.text()).toMatch(
      /^Plain Minutes of m2 at UTC660 (AEDT|GMT\+11)$/,
    )
  })

  it('copies plain text only', async () => {
    const { reader } = await openReader()
    await userEvent.click(within(reader).getByRole('button', { name: 'Copy plain text' }))
    await waitFor(() => expect(write).toHaveBeenCalledOnce())
    const [item] = write.mock.calls[0]![0] as { items: Record<string, Blob> }[]
    expect(Object.keys(item!.items)).toEqual(['text/plain'])
    expect(await item!.items['text/plain']!.text()).toMatch(/^Plain Minutes of m2 at UTC660 /)
  })
})
