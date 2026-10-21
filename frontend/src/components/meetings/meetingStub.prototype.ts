// PROTOTYPE (Meetings sidebar UI, #75): throwaway. An in-memory stand-in for
// the Meeting Lock and Meetings API so the sidebar variants can be judged.
// Nothing here hits the server; "Simulate reload" mimics the localStorage resume.
import { computed, onBeforeUnmount, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'

export const VARIANTS = {
  A: 'Chosen: sidebar + reader',
  B: 'Docked rail beside the page',
  C: 'Meeting bar + reader',
  D: 'Chosen so far: rail + reader',
} as const
export type Variant = keyof typeof VARIANTS

export function useVariant() {
  const route = useRoute()
  return computed<Variant>(() => {
    const v = String(route.query.variant ?? 'D')
    return v in VARIANTS ? (v as Variant) : 'D'
  })
}

export interface StubComment {
  id: number
  body: string
  at: Date
}

export interface PastMeeting {
  id: string
  leader: string
  attendees: string[]
  startedAt: Date
  endedAt: Date
  comments: string[]
}

type Role = 'leader' | 'member' | 'other-leader-running'

const ago = (mins: number) => new Date(Date.now() - mins * 60_000)

export const stub = reactive({
  role: 'leader' as Role,
  /** The running Meeting, as the holding session's localStorage would hold it. */
  running: null as null | { startedAt: Date; attendeeIds: string[]; comments: StubComment[] },
  /** Set by "Simulate reload": the variant must offer to resume. */
  resumePending: false,
  /** The Meeting just created, so the variant can show its Minutes straight away. */
  justEnded: null as null | PastMeeting,
  past: [
    {
      id: 'm3',
      leader: 'Priya Shah',
      attendees: ['Priya Shah', 'Tom Nguyen', 'Alex Reid'],
      startedAt: ago(7 * 24 * 60 + 35),
      endedAt: ago(7 * 24 * 60),
      comments: ['Vendor confirmed the March date.', 'Budget review moved to next week.'],
    },
    {
      id: 'm2',
      leader: 'Priya Shah',
      attendees: ['Priya Shah', 'Tom Nguyen'],
      startedAt: ago(14 * 24 * 60 + 50),
      endedAt: ago(14 * 24 * 60),
      comments: ['Agreed to escalate the data-centre Blocker.'],
    },
    {
      id: 'm1',
      leader: 'Sam Okafor',
      attendees: ['Sam Okafor', 'Priya Shah', 'Tom Nguyen', 'Alex Reid', 'Jo Bloggs'],
      startedAt: ago(21 * 24 * 60 + 60),
      endedAt: ago(21 * 24 * 60),
      comments: [],
    },
  ] as PastMeeting[],
})

let nextId = 1

export function start(attendeeIds: string[]) {
  stub.justEnded = null
  stub.running = { startedAt: new Date(), attendeeIds, comments: [] }
}

export function addComment(body: string) {
  if (stub.running && body.trim())
    stub.running.comments.push({ id: nextId++, body: body.trim(), at: new Date() })
}

export function removeComment(id: number) {
  if (stub.running) stub.running.comments = stub.running.comments.filter((c) => c.id !== id)
}

export function cancel() {
  stub.running = null
  stub.resumePending = false
}

export function end(names: (id: string) => string, leader: string) {
  const r = stub.running
  if (!r) return
  const meeting: PastMeeting = {
    id: `m${stub.past.length + 1}`,
    leader,
    attendees: [leader, ...r.attendeeIds.map(names).filter((n) => n !== leader)],
    startedAt: r.startedAt,
    endedAt: new Date(),
    comments: r.comments.map((c) => c.body),
  }
  stub.past.unshift(meeting)
  stub.running = null
  stub.resumePending = false
  stub.justEnded = meeting
}

export function simulateReload() {
  if (stub.running) stub.resumePending = true
}

export const fmtTime = (d: Date) =>
  d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
export const fmtDate = (d: Date) =>
  d.toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'short' })
export const minutesLong = (m: PastMeeting) =>
  Math.max(1, Math.round((+m.endedAt - +m.startedAt) / 60_000))

/** A rough stand-in for the server's `text/plain` render of the Minutes (#68). */
export function minutesText(m: PastMeeting, project: string) {
  const lines = [
    project,
    `${fmtDate(m.startedAt)}, ${fmtTime(m.startedAt)}–${fmtTime(m.endedAt)}`,
    `Led by ${m.leader}. Attendees: ${m.attendees.join(', ')}`,
    '',
    'ACTIONS',
    '  New',
    '    A-14 Book venue for launch (Tom Nguyen, due 3 Nov)',
    '  Updated',
    '    A-9 Draft comms plan',
    '      - Due date 24 Oct → 31 Oct',
    '      - "Waiting on legal sign-off."',
    '',
    'BLOCKERS',
    '  Updated',
    '    B-3 Data-centre access',
    '      - Cleared',
    '',
    'MEETING',
    ...(m.comments.length ? m.comments.map((c) => `  - ${c}`) : ['  (no Comments)']),
  ]
  return lines.join('\n')
}

export function minutesHtml(m: PastMeeting, project: string) {
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;')
  return `<h2>${esc(project)}</h2>
<p><small>${fmtDate(m.startedAt)}, ${fmtTime(m.startedAt)}–${fmtTime(m.endedAt)}</small></p>
<p>Led by <b>${esc(m.leader)}</b>. Attendees: ${m.attendees.map(esc).join(', ')}</p>
<h3>Actions</h3><h4>New</h4><ul><li><b>A-14</b> Book venue for launch (Tom Nguyen, due 3 Nov)</li></ul>
<h4>Updated</h4><p><b>A-9</b> Draft comms plan</p><ul><li>Due date 24 Oct → 31 Oct</li><li><i>"Waiting on legal sign-off."</i></li></ul>
<h3>Blockers</h3><h4>Updated</h4><p><b>B-3</b> Data-centre access</p><ul><li>Cleared</li></ul>
<h3>Meeting</h3><ul>${m.comments.map((c) => `<li>${esc(c)}</li>`).join('') || '<li>(no Comments)</li>'}</ul>`
}

export async function copyMinutes(m: PastMeeting, project: string, as: 'text' | 'html') {
  const text = minutesText(m, project)
  if (as === 'text') return navigator.clipboard.writeText(text)
  return navigator.clipboard.write([
    new ClipboardItem({
      'text/html': new Blob([minutesHtml(m, project)], { type: 'text/html' }),
      'text/plain': new Blob([text], { type: 'text/plain' }),
    }),
  ])
}

export function useElapsed(from: () => Date | undefined) {
  const now = ref(Date.now())
  const t = setInterval(() => (now.value = Date.now()), 1000)
  onBeforeUnmount(() => clearInterval(t))
  return computed(() => {
    const f = from()
    if (!f) return ''
    const s = Math.max(0, Math.floor((now.value - +f) / 1000))
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
  })
}
