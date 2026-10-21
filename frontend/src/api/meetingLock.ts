import { ApiError, client, unwrap } from './client'
import type { components } from './schema'

export type MeetingLock = components['schemas']['MeetingLockBody']

/** A Project's Meeting Lock, or `null` when there is none. */
export async function getMeetingLock(projectId: string): Promise<MeetingLock | null> {
  try {
    return unwrap(
      await client.GET('/api/v1/projects/{id}/meeting-lock', {
        params: { path: { id: projectId } },
      }),
    )
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return null
    throw e
  }
}

/** Keeps this session's Meeting Lock live; the leader's page calls it every 30 seconds. */
export async function heartbeatMeetingLock(projectId: string): Promise<MeetingLock> {
  return unwrap(
    await client.PUT('/api/v1/projects/{id}/meeting-lock', {
      params: { path: { id: projectId } },
    }),
  )
}

/** Releases this session's Meeting Lock without creating a Meeting. */
export async function cancelMeetingLock(projectId: string): Promise<void> {
  unwrap(
    await client.DELETE('/api/v1/projects/{id}/meeting-lock', {
      params: { path: { id: projectId } },
    }),
  )
}

/** Acquires a Project's Meeting Lock for this session (Project Leader only). */
export async function acquireMeetingLock(projectId: string): Promise<MeetingLock> {
  return unwrap(
    await client.POST('/api/v1/projects/{id}/meeting-lock', {
      params: { path: { id: projectId } },
    }),
  )
}
