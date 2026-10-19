import { http, HttpResponse } from 'msw'
import type { components } from '../api/schema'
import { api, problem } from './server'

type Schemas = components['schemas']
export type Action = Schemas['Action']
type Mutable<T> = { -readonly [K in keyof T]: T[K] }

/** The `/actions` endpoints of the fake backend, over the given Projects and Members. */
export function fakeActions(
  projects: Schemas['Project'][],
  members: Map<string, Set<string>>,
  personRef: (id: string) => Schemas['PersonRef'],
  initial: Action[],
  blockedBy: (actionId: string) => Schemas['ItemRef'][] = () => [],
) {
  const actions = initial.map((a) => ({ ...a, blocked_by: [] }) as Mutable<Action>)
  /** As served: with the open Blockers blocking it (FR-53). */
  const shown = (a: Action): Action => ({ ...a, blocked_by: blockedBy(a.id) })
  let n = 0

  const isMember = (projectId: string, a: Action['assignee']) =>
    !a || (members.get(projectId)?.has(a.id) ?? false)

  // Finds an Action that may be changed with `If-Match`, or the problem to return.
  function writable(request: Request, id: unknown): Mutable<Action> | Response {
    const action = actions.find((a) => a.id === id)
    if (!action) return problem(404, 'Not Found')
    if (request.headers.get('If-Match') !== `"${action.version}"`)
      return problem(412, 'Precondition Failed')
    if (projects.find((p) => p.id === action.project_id)?.archived) return problem(409, 'Conflict')
    return action
  }

  /** As removing a Member does: their open Actions become unassigned. */
  function unassign(projectId: string, personId: string) {
    for (const a of actions)
      if (a.project_id === projectId && !a.closed && a.assignee?.id === personId) {
        a.assignee = null
        a.version += 1
      }
  }

  const handlers = [
    http.get(api('/actions'), ({ request }) => {
      const query = new URL(request.url).searchParams
      const items = actions.filter(
        (a) =>
          (!query.has('project_id') || a.project_id === query.get('project_id')) &&
          (query.get('include_closed') === 'true' || !a.closed),
      )
      return HttpResponse.json<Schemas['ActionPage']>({
        items: items.map(shown),
        limit: 1000,
        offset: 0,
        total: items.length,
      })
    }),
    http.post(api('/actions'), async ({ request }) => {
      const body = (await request.json()) as Action
      const project = projects.find((p) => p.id === body.project_id)
      if (!project || !body.description.trim() || !isMember(project.id, body.assignee ?? null))
        return problem(422, 'Unprocessable Content')
      if (project.archived) return problem(409, 'Conflict')
      const number = actions.filter((a) => a.project_id === project.id).length + 1
      const action: Mutable<Action> = {
        id: `action-${++n}`,
        project_id: project.id,
        project_code: project.project_code,
        reference: `A-${String(number).padStart(3, '0')}`,
        description: body.description.trim(),
        assignee: body.assignee ? personRef(body.assignee.id) : null,
        due_date: body.due_date ?? null,
        closed: false,
        blocked_by: [],
        version: 1,
      }
      actions.push(action)
      return HttpResponse.json<Action>(shown(action), { status: 201 })
    }),
    http.put(api('/actions/:id'), async ({ request, params }) => {
      const action = writable(request, params.id)
      if (action instanceof Response) return action
      const body = (await request.json()) as Action
      if (!('assignee' in body) || !('due_date' in body) || !body.description.trim())
        return problem(422, 'Unprocessable Content')
      if (body.assignee?.id !== action.assignee?.id && !isMember(action.project_id, body.assignee))
        return problem(422, 'Unprocessable Content')
      action.description = body.description.trim()
      action.assignee = body.assignee ? personRef(body.assignee.id) : null
      action.due_date = body.due_date
      action.version += 1
      return HttpResponse.json<Action>(shown(action))
    }),
    http.patch(api('/actions/:id'), async ({ request, params }) => {
      if (request.headers.get('Content-Type') !== 'application/merge-patch+json')
        return problem(415, 'Unsupported Media Type')
      const action = writable(request, params.id)
      if (action instanceof Response) return action
      const { closed } = (await request.json()) as Schemas['ActionPatch']
      if (action.closed !== closed) {
        action.closed = closed
        if (!closed && !isMember(action.project_id, action.assignee)) action.assignee = null
        action.version += 1
      }
      return HttpResponse.json<Action>(shown(action))
    }),
  ]
  return { handlers, actions, unassign }
}
