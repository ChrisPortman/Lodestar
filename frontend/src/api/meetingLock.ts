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

/** Acquires a Project's Meeting Lock for this session (Project Leader only). */
export async function acquireMeetingLock(projectId: string): Promise<MeetingLock> {
  return unwrap(
    await client.POST('/api/v1/projects/{id}/meeting-lock', {
      params: { path: { id: projectId } },
    }),
  )
}
