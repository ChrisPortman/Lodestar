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

/** A Meeting's Minutes as plain text, as the server renders them. */
export function useMinutesText(meetingId: Ref<string>, enabled: Ref<boolean>) {
  return useQuery({
    queryKey: computed(() => ['minutes', meetingId.value, 'text/plain'] as const),
    queryFn: () => getMinutes(meetingId.value, 'text/plain'),
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

/** "Wed 21 Oct 2026, 09:00–10:15 UTC (75 min)", in UTC like the server's renderings. */
export function minutesWhen(minutes: Minutes): string {
  const start = new Date(minutes.start)
  const end = new Date(minutes.end)
  const part = (options: Intl.DateTimeFormatOptions) =>
    new Intl.DateTimeFormat('en-GB', { ...options, timeZone: 'UTC' }).format(start)
  const pad = (n: number) => String(n).padStart(2, '0')
  const time = (at: Date) => `${pad(at.getUTCHours())}:${pad(at.getUTCMinutes())}`
  const date = `${part({ weekday: 'short' })} ${start.getUTCDate()} ${part({ month: 'short' })} ${start.getUTCFullYear()}`
  return `${date}, ${time(start)}–${time(end)} UTC (${minutes.duration_minutes} min)`
}

/**
 * Puts a Meeting's Minutes on the clipboard: formatted (HTML with a plain-text
 * fallback) or plain text only.
 */
export async function copyMinutes(meetingId: string, formatted: boolean): Promise<void> {
  const types: MinutesType[] = formatted ? ['text/html', 'text/plain'] : ['text/plain']
  const blobs = await Promise.all(
    types.map(async (type) => [type, new Blob([await getMinutes(meetingId, type)], { type })]),
  )
  await navigator.clipboard.write([new ClipboardItem(Object.fromEntries(blobs))])
}

/** "Wed 21 Oct 2026, 09:00" in local time. */
export function meetingStart(meeting: MeetingSummary): string {
  const at = new Date(meeting.started_at)
  const part = (options: Intl.DateTimeFormatOptions) =>
    new Intl.DateTimeFormat('en-GB', options).format(at)
  const pad = (n: number) => String(n).padStart(2, '0')
  const date = `${part({ weekday: 'short' })} ${at.getDate()} ${part({ month: 'short' })} ${at.getFullYear()}`
  return `${date}, ${pad(at.getHours())}:${pad(at.getMinutes())}`
}

/** Whole minutes from start to end. */
export function meetingLength(meeting: MeetingSummary): number {
  return Math.round((Date.parse(meeting.ended_at) - Date.parse(meeting.started_at)) / 60_000)
}
