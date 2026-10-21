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

/** New and Updated items of one type in the Minutes. */
export interface MinutesSection {
  new?: { reference: string; title: string; state?: string[]; comments?: string[] }[]
  updated?: { reference: string; title: string; changes?: string[]; comments?: string[] }[]
}

/** The stored Minutes document (`schema` 1), every name already resolved. */
export interface Minutes {
  schema: number
  project_name: string
  start: string
  end: string
  duration_minutes: number
  leader: string
  attendees: string[]
  actions?: MinutesSection
  risks?: MinutesSection
  blockers?: MinutesSection
  meeting_comments?: string[]
}

/** A Meeting's Minutes as the stored JSON document. */
export async function getMinutesDocument(meetingId: string): Promise<Minutes> {
  const result = await client.GET('/api/v1/meetings/{id}/minutes', {
    params: { path: { id: meetingId } },
    headers: { Accept: 'application/json' },
  })
  // The OpenAPI schema leaves the document untyped; `Minutes` mirrors core::minutes.
  return unwrap(result) as unknown as Minutes
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
