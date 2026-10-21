// Meetings in the Project menu (#84): starting one, Past Meetings and the Minutes.
import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { acquireMeetingLock, getMeetingLock, type MeetingLock } from './api/meetingLock'
import {
  getMinutes,
  getMinutesDocument,
  listMeetings,
  type MeetingSummary,
  type Minutes,
  type MinutesSection,
  type MinutesType,
  type ReaderZone,
} from './api/meetings'
import { meetingLockKey } from './meetingLock'

export const meetingsKey = (projectId: string) => ['meetings', projectId] as const

/** The Project's Meeting Lock, sharing its cache with the Meeting in progress banner. */
export function useMeetingLock(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => meetingLockKey(projectId.value)),
    queryFn: () => getMeetingLock(projectId.value),
  })
}

/**
 * Starts a Meeting by acquiring the Project's Meeting Lock. `onStarted` runs
 * before the lock reaches the cache, so the page knows the Meeting is new
 * rather than one to resume.
 */
export function useStartMeeting(
  projectId: Ref<string>,
  onStarted: (lock: MeetingLock) => void = () => {},
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => acquireMeetingLock(projectId.value),
    onSuccess: (lock) => {
      onStarted(lock)
      queryClient.setQueryData(meetingLockKey(projectId.value), lock)
    },
    // A 409 means someone else got there first: show who.
    onError: () => queryClient.invalidateQueries({ queryKey: meetingLockKey(projectId.value) }),
  })
}

export function useMeetings(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => meetingsKey(projectId.value)),
    queryFn: () => listMeetings(projectId.value),
  })
}

/** A Meeting's Minutes as plain text, as the server renders them in local time. */
export function useMinutesText(meeting: Ref<MeetingSummary>, enabled: Ref<boolean>) {
  return useQuery({
    queryKey: computed(() => ['minutes', meeting.value.id, 'text/plain'] as const),
    queryFn: () =>
      getMinutes(meeting.value.id, 'text/plain', readerZone(new Date(meeting.value.started_at))),
    enabled,
    staleTime: Infinity, // Meetings never change
  })
}

/** A Meeting's Minutes document, for the Formatted preview to lay out. */
export function useMinutesDocument(meetingId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => ['minutes', meetingId.value, 'application/json'] as const),
    queryFn: () => getMinutesDocument(meetingId.value),
    staleTime: Infinity,
  })
}

/** One section of the Minutes, laid out as the server's renderings do (#83). */
export interface MinutesPart {
  name: string
  new: NonNullable<MinutesSection['new']>
  updated: NonNullable<MinutesSection['updated']>
}

/** The Minutes' non-empty Action, Risk and Blocker sections, in order. */
export function minutesParts(minutes: Minutes): MinutesPart[] {
  const parts: [string, MinutesSection | undefined][] = [
    ['Action', minutes.actions],
    ['Risk', minutes.risks],
    ['Blocker', minutes.blockers],
  ]
  return parts
    .map(([name, s]) => ({ name, new: s?.new ?? [], updated: s?.updated ?? [] }))
    .filter((p) => p.new.length || p.updated.length)
}

/** The browser's short name for its zone at `at`, e.g. "AEDT" (or "GMT+11"). */
function zoneName(at: Date): string {
  const parts = new Intl.DateTimeFormat(undefined, { timeZoneName: 'short' }).formatToParts(at)
  return parts.find((p) => p.type === 'timeZoneName')?.value ?? ''
}

/**
 * The browser's zone as it stood at `at` (a Meeting's start), for the server
 * to render Minutes in. Times are stored in UTC; people read them locally.
 */
export function readerZone(at: Date): ReaderZone {
  return { tz_offset: -at.getTimezoneOffset(), tz_name: zoneName(at) }
}

/** "Wed 21 Oct 2026, 20:00–21:15 AEDT (75 min)", in local time like the server's renderings. */
export function minutesWhen(minutes: Minutes): string {
  const start = new Date(minutes.start)
  const end = new Date(minutes.end)
  const pad = (n: number) => String(n).padStart(2, '0')
  const time = (at: Date) => `${pad(at.getHours())}:${pad(at.getMinutes())}`
  return `${localDate(start)}, ${time(start)}–${time(end)} ${zoneName(start)} (${minutes.duration_minutes} min)`
}

/** "Wed 21 Oct 2026" in local time. */
function localDate(at: Date): string {
  const part = (options: Intl.DateTimeFormatOptions) =>
    new Intl.DateTimeFormat('en-GB', options).format(at)
  return `${part({ weekday: 'short' })} ${at.getDate()} ${part({ month: 'short' })} ${at.getFullYear()}`
}

/**
 * Puts a Meeting's Minutes on the clipboard, rendered in local time: formatted
 * (HTML with a plain-text fallback) or plain text only.
 */
export async function copyMinutes(meeting: MeetingSummary, formatted: boolean): Promise<void> {
  const zone = readerZone(new Date(meeting.started_at))
  const types: MinutesType[] = formatted ? ['text/html', 'text/plain'] : ['text/plain']
  const blobs = await Promise.all(
    types.map(async (type) => [
      type,
      new Blob([await getMinutes(meeting.id, type, zone)], { type }),
    ]),
  )
  await navigator.clipboard.write([new ClipboardItem(Object.fromEntries(blobs))])
}

/** "Wed 21 Oct 2026, 09:00" in local time. */
export function meetingStart(meeting: MeetingSummary): string {
  const at = new Date(meeting.started_at)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${localDate(at)}, ${pad(at.getHours())}:${pad(at.getMinutes())}`
}

/** Whole minutes from start to end. */
export function meetingLength(meeting: MeetingSummary): number {
  return Math.round((Date.parse(meeting.ended_at) - Date.parse(meeting.started_at)) / 60_000)
}
