import { client, ifMatch, unwrap } from './client'
import type { components } from './schema'

export type Blocker = components['schemas']['Blocker']

/** Every Blocker in a Project, cleared ones included, in Reference order. */
export async function listBlockers(projectId: string): Promise<Blocker[]> {
  const page = unwrap(
    await client.GET('/api/v1/blockers', {
      params: { query: { project_id: projectId, state: 'all', sort: 'reference', limit: 1000 } },
    }),
  )
  return page.items
}

export interface BlockerFields {
  description: string
  accountable: { id: string; display_name: string } | null
}

export async function createBlocker(projectId: string, fields: BlockerFields): Promise<Blocker> {
  return unwrap(
    await client.POST('/api/v1/blockers', {
      body: { project_id: projectId, ...fields } as Blocker,
    }),
  )
}

/** Replaces the editable fields, keeping the rest as read. */
export async function updateBlocker(
  blocker: Blocker,
  fields: Partial<BlockerFields>,
): Promise<Blocker> {
  return unwrap(
    await client.PUT('/api/v1/blockers/{id}', {
      params: { path: { id: blocker.id }, header: { 'if-match': ifMatch(blocker.version) } },
      body: { ...blocker, ...fields },
    }),
  )
}

export async function setBlockerCleared(blocker: Blocker, cleared: boolean): Promise<Blocker> {
  return unwrap(
    await client.PATCH('/api/v1/blockers/{id}', {
      params: { path: { id: blocker.id }, header: { 'if-match': ifMatch(blocker.version) } },
      headers: { 'Content-Type': 'application/merge-patch+json' },
      body: { cleared },
    }),
  )
}

export async function linkAction(blocker: Blocker, actionId: string): Promise<Blocker> {
  return unwrap(
    await client.POST('/api/v1/blockers/{id}/blocked_actions', {
      params: { path: { id: blocker.id }, header: { 'if-match': ifMatch(blocker.version) } },
      body: { action_id: actionId },
    }),
  )
}

export async function unlinkAction(blocker: Blocker, actionId: string): Promise<Blocker> {
  return unwrap(
    await client.DELETE('/api/v1/blockers/{id}/blocked_actions/{action_id}', {
      params: {
        path: { id: blocker.id, action_id: actionId },
        header: { 'if-match': ifMatch(blocker.version) },
      },
    }),
  )
}
