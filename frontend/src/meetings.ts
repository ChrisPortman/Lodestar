// Meetings in the Project menu (#84): starting one, Past Meetings and the Minutes.
import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { acquireMeetingLock, getMeetingLock } from './api/meetingLock'
import { getMinutes, listMeetings, type MeetingSummary, type MinutesType } from './api/meetings'
import { meetingLockKey } from './meetingLock'

export const meetingsKey = (projectId: string) => ['meetings', projectId] as const

/** The Project's Meeting Lock, sharing its cache with the Meeting in progress banner. */
export function useMeetingLock(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => meetingLockKey(projectId.value)),
    queryFn: () => getMeetingLock(projectId.value),
  })
}

/** Starts a Meeting by acquiring the Project's Meeting Lock. */
export function useStartMeeting(projectId: Ref<string>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => acquireMeetingLock(projectId.value),
    onSuccess: (lock) => queryClient.setQueryData(meetingLockKey(projectId.value), lock),
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

export function useMinutes(meetingId: Ref<string>, type: Ref<MinutesType>) {
  return useQuery({
    queryKey: computed(() => ['minutes', meetingId.value, type.value] as const),
    queryFn: () => getMinutes(meetingId.value, type.value),
    staleTime: Infinity, // Meetings never change
  })
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
