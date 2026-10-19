import { client, ifMatch, unwrap } from './client'
import type { components } from './schema'

export type Action = components['schemas']['Action']

/** Every Action in a Project, closed ones included, in Reference order. */
export async function listActions(projectId: string): Promise<Action[]> {
  const page = unwrap(
    await client.GET('/api/v1/actions', {
      params: { query: { project_id: projectId, state: 'all', limit: 1000 } },
    }),
  )
  return page.items
}

export interface ActionFields {
  description: string
  assignee: { id: string; display_name: string } | null
  due_date: string | null
}

export async function createAction(projectId: string, fields: ActionFields): Promise<Action> {
  return unwrap(
    await client.POST('/api/v1/actions', {
      body: { project_id: projectId, ...fields } as Action,
    }),
  )
}

/** Replaces the editable fields, keeping the rest as read. */
export async function updateAction(action: Action, fields: Partial<ActionFields>): Promise<Action> {
  return unwrap(
    await client.PUT('/api/v1/actions/{id}', {
      params: { path: { id: action.id }, header: { 'if-match': ifMatch(action.version) } },
      body: { ...action, ...fields },
    }),
  )
}

export async function setActionClosed(action: Action, closed: boolean): Promise<Action> {
  return unwrap(
    await client.PATCH('/api/v1/actions/{id}', {
      params: { path: { id: action.id }, header: { 'if-match': ifMatch(action.version) } },
      headers: { 'Content-Type': 'application/merge-patch+json' },
      body: { closed },
    }),
  )
}
