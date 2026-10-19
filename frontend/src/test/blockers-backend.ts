import { http, HttpResponse } from 'msw'
import type { components } from '../api/schema'
import { api, problem } from './server'
import type { Action } from './actions-backend'

type Schemas = components['schemas']
export type Blocker = Schemas['Blocker']
type ItemRef = Schemas['ItemRef']
type Mutable<T> = { -readonly [K in keyof T]: T[K] }

/** The `/blockers` endpoints of the fake backend, over the given Projects, Members and Actions. */
export function fakeBlockers(
  projects: Schemas['Project'][],
  members: Map<string, Set<string>>,
  personRef: (id: string) => Schemas['PersonRef'],
  actions: Action[],
  initial: Blocker[],
) {
  const blockers = initial.map((b) => ({ ...b }) as Mutable<Blocker>)
  let n = 0

  const isMember = (projectId: string, a: Blocker['accountable']) =>
    !a || (members.get(projectId)?.has(a.id) ?? false)

  // Finds a Blocker that may be changed with `If-Match`, or the problem to return.
  function writable(request: Request, id: unknown): Mutable<Blocker> | Response {
    const blocker = blockers.find((b) => b.id === id)
    if (!blocker) return problem(404, 'Not Found')
    if (request.headers.get('If-Match') !== `"${blocker.version}"`)
      return problem(412, 'Precondition Failed')
    if (projects.find((p) => p.id === blocker.project_id)?.archived) return problem(409, 'Conflict')
    return blocker
  }

  /** As removing a Member does: their open Blockers lose their accountable Member. */
  function unassign(projectId: string, personId: string) {
    for (const b of blockers)
      if (b.project_id === projectId && !b.cleared && b.accountable?.id === personId) {
        b.accountable = null
        b.version += 1
      }
  }

  /** The open Blockers blocking an Action (FR-53). */
  function blockedBy(actionId: string): ItemRef[] {
    return blockers
      .filter((b) => !b.cleared && b.blocked_actions.some((a) => a.id === actionId))
      .map((b) => ({ id: b.id, reference: b.reference }))
  }

  const handlers = [
    http.get(api('/blockers'), ({ request }) => {
      const query = new URL(request.url).searchParams
      const inState = (state: string, cleared: boolean) =>
        state === 'all' || (state === 'cleared') === cleared
      const items = blockers.filter(
        (b) =>
          (!query.has('project_id') || b.project_id === query.get('project_id')) &&
          inState(query.get('state') ?? 'open', b.cleared),
      )
      return HttpResponse.json<Schemas['BlockerPage']>({
        items,
        limit: 1000,
        offset: 0,
        total: items.length,
      })
    }),
    http.post(api('/blockers'), async ({ request }) => {
      const body = (await request.json()) as Blocker
      const project = projects.find((p) => p.id === body.project_id)
      if (!project || !body.description.trim() || !isMember(project.id, body.accountable ?? null))
        return problem(422, 'Unprocessable Content')
      if (project.archived) return problem(409, 'Conflict')
      const number = blockers.filter((b) => b.project_id === project.id).length + 1
      const blocker: Mutable<Blocker> = {
        id: `blocker-${++n}`,
        project_id: project.id,
        project_code: project.project_code,
        reference: `B-${String(number).padStart(3, '0')}`,
        description: body.description.trim(),
        accountable: body.accountable ? personRef(body.accountable.id) : null,
        cleared: false,
        blocked_actions: [],
        version: 1,
      }
      blockers.push(blocker)
      return HttpResponse.json<Blocker>(blocker, { status: 201 })
    }),
    http.put(api('/blockers/:id'), async ({ request, params }) => {
      const blocker = writable(request, params.id)
      if (blocker instanceof Response) return blocker
      const body = (await request.json()) as Blocker
      if (!('accountable' in body) || !body.description.trim())
        return problem(422, 'Unprocessable Content')
      if (
        body.accountable?.id !== blocker.accountable?.id &&
        !isMember(blocker.project_id, body.accountable)
      )
        return problem(422, 'Unprocessable Content')
      blocker.description = body.description.trim()
      blocker.accountable = body.accountable ? personRef(body.accountable.id) : null
      blocker.version += 1
      return HttpResponse.json<Blocker>(blocker)
    }),
    http.patch(api('/blockers/:id'), async ({ request, params }) => {
      if (request.headers.get('Content-Type') !== 'application/merge-patch+json')
        return problem(415, 'Unsupported Media Type')
      const blocker = writable(request, params.id)
      if (blocker instanceof Response) return blocker
      const { cleared } = (await request.json()) as Schemas['BlockerPatch']
      if (blocker.cleared !== cleared) {
        blocker.cleared = cleared
        if (!cleared && !isMember(blocker.project_id, blocker.accountable))
          blocker.accountable = null
        blocker.version += 1
      }
      return HttpResponse.json<Blocker>(blocker)
    }),
    http.post(api('/blockers/:id/blocked_actions'), async ({ request, params }) => {
      const blocker = writable(request, params.id)
      if (blocker instanceof Response) return blocker
      const { action_id } = (await request.json()) as Schemas['NewBlockedAction']
      const action = actions.find((a) => a.id === action_id)
      if (!action || action.project_id !== blocker.project_id)
        return problem(422, 'Unprocessable Content')
      if (!blocker.blocked_actions.some((a) => a.id === action_id)) {
        blocker.blocked_actions = [
          ...blocker.blocked_actions,
          { id: action.id, reference: action.reference },
        ].sort((a, b) => a.reference.localeCompare(b.reference))
        blocker.version += 1
      }
      return HttpResponse.json<Blocker>(blocker)
    }),
    http.delete(api('/blockers/:id/blocked_actions/:actionId'), ({ request, params }) => {
      const blocker = writable(request, params.id)
      if (blocker instanceof Response) return blocker
      if (blocker.blocked_actions.some((a) => a.id === params.actionId)) {
        blocker.blocked_actions = blocker.blocked_actions.filter((a) => a.id !== params.actionId)
        blocker.version += 1
      }
      return HttpResponse.json<Blocker>(blocker)
    }),
  ]
  return { handlers, blockers, unassign, blockedBy }
}
