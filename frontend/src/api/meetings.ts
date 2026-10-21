import { client, unwrap } from './client'
import type { components } from './schema'

export type MeetingSummary = components['schemas']['MeetingSummaryBody']
export type MinutesType = 'text/html' | 'text/plain'

export type Meeting = components['schemas']['Meeting']
export type NewMeeting = components['schemas']['NewMeetingBody']

/** Ends the Meeting this session holds the lock for, creating its Minutes. */
export async function createMeeting(body: NewMeeting): Promise<Meeting> {
  return unwrap(await client.POST('/api/v1/meetings', { body }))
}

/** A Project's Meetings, newest first. */
export async function listMeetings(projectId: string): Promise<MeetingSummary[]> {
  const page = unwrap(
    await client.GET('/api/v1/meetings', {
      params: { query: { project_id: projectId, limit: 1000 } },
    }),
  )
  return page.items
}

/** A Meeting's Minutes, as the server renders them for `type`. */
export async function getMinutes(meetingId: string, type: MinutesType): Promise<string> {
  return unwrap(
    await client.GET('/api/v1/meetings/{id}/minutes', {
      params: { path: { id: meetingId } },
      headers: { Accept: type },
      parseAs: 'text',
    }),
  )
}
