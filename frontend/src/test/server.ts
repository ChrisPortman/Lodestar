import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'
import type { components } from '../api/schema'
import { fakeActions, type Action } from './actions-backend'
import { fakeBlockers, type Blocker } from './blockers-backend'
import { fakeComments, type Comment } from './comments-backend'
import { fakeRisks, type Risk } from './risks-backend'

export type Person = components['schemas']['Person']
export type Project = components['schemas']['Project']
export type MeetingLock = components['schemas']['MeetingLockBody']
export type MeetingSummary = components['schemas']['MeetingSummaryBody']
export type { Action, Blocker, Comment, Risk }
type Schemas = components['schemas']

export const api = (path: string) => `${location.origin}/api/v1${path}`

// Typed as a plain Response so resolvers may return either a body or a problem.
export function problem(status: number, title: string): Response {
  return HttpResponse.json<Schemas['Problem']>(
    { type: 'about:blank', status, title },
    { status, headers: { 'Content-Type': 'application/problem+json' } },
  )
}

type Mutable<T> = { -readonly [K in keyof T]: T[K] }

/** A crude stand-in for FR-16: the first four letters, numbered if taken. */
function suggest(name: string, taken: string[]): string | null {
  const base = name
    .replace(/[^a-z]/gi, '')
    .slice(0, 4)
    .toUpperCase()
  if (base.length < 2) return null
  let code = base
  for (let n = 1; taken.includes(code); n++) code = `${base}${n}`
  return code
}

/** An in-memory backend for the People, Projects and session endpoints. */
export function fakeBackend(
  initial: Person[],
  actingId: string | null = null,
  initialProjects: Project[] = [],
  initialRisks: Risk[] = [],
  initialActions: Action[] = [],
  initialBlockers: Blocker[] = [],
  initialComments: Comment[] = [],
  initialLock: MeetingLock | null = null,
) {
  // One Meeting Lock, as the acting session sees it, for whichever Project is viewed.
  let lock = initialLock
  // Past Meetings, in any order; the server lists them newest first.
  const meetings: MeetingSummary[] = []
  // The bodies of Meetings created through the API, oldest first.
  const created: Schemas['NewMeetingBody'][] = []
  let heartbeats = 0
  const lockedOut = () => lock !== null && lock.live && !lock.held_by_you
  const people = initial.map((p) => ({ ...p }))
  const projects = initialProjects.map((p) => ({ ...p }) as Mutable<Project>)
  // Member IDs per Project; each Project starts with its leader as its only Member.
  const members = new Map(
    projects.map((p) => [p.id, new Set(p.project_leader ? [p.project_leader.id] : [])]),
  )
  let acting = actingId
  let n = 0
  const personRef = (id: string) => {
    const person = people.find((p) => p.id === id)
    return { id, display_name: person?.display_name ?? '' }
  }
  /** The writable Project, or the problem that stops it changing. */
  const writable = (id: unknown, request: Request): Mutable<Project> | Response => {
    const project = projects.find((p) => p.id === id)
    if (!project) return problem(404, 'Not Found')
    if (request.headers.get('If-Match') !== `"${project.version}"`)
      return problem(412, 'Precondition Failed')
    if (project.archived) return problem(409, 'Conflict')
    return project
  }
  const handlers = [
    http.get(api('/people'), () =>
      HttpResponse.json<Schemas['PersonPage']>({
        items: people,
        limit: 100,
        offset: 0,
        total: people.length,
      }),
    ),
    http.post(api('/people'), async ({ request }) => {
      const body = (await request.json()) as Person
      const display_name = body.display_name.trim()
      if (!display_name) return problem(422, 'Unprocessable Content')
      const person = { id: `new-${++n}`, version: 1, display_name }
      people.push(person)
      acting ??= person.id // the server starts a session only when there is none
      return HttpResponse.json<Person>(person, { status: 201 })
    }),
    http.put(api('/people/:id'), async ({ request, params }) => {
      const person = people.find((p) => p.id === params.id)
      if (!person) return problem(404, 'Not Found')
      if (request.headers.get('If-Match') !== `"${person.version}"`)
        return problem(412, 'Precondition Failed')
      const body = (await request.json()) as Person
      person.display_name = body.display_name.trim()
      person.version += 1
      return HttpResponse.json<Person>(person)
    }),
    http.get(api('/projects/code_suggestion'), ({ request }) => {
      const name = new URL(request.url).searchParams.get('name') ?? ''
      const project_code = suggest(
        name,
        projects.map((p) => p.project_code),
      )
      return HttpResponse.json<Schemas['CodeSuggestion']>({ project_code })
    }),
    http.get(api('/projects'), ({ request }) => {
      const all = new URL(request.url).searchParams.get('include_archived') === 'true'
      const items = projects.filter((p) => all || !p.archived)
      return HttpResponse.json<Schemas['ProjectPage']>({
        items,
        limit: 100,
        offset: 0,
        total: items.length,
      })
    }),
    http.post(api('/projects'), async ({ request }) => {
      const body = (await request.json()) as Project
      if (projects.some((p) => p.project_code === body.project_code))
        return problem(409, 'Conflict')
      const person = people.find((p) => p.id === acting)
      const project: Mutable<Project> = {
        id: `project-${++n}`,
        version: 1,
        name: body.name.trim(),
        description: body.description?.trim() || null,
        project_code: body.project_code,
        archived: false,
        project_leader: person ? { id: person.id, display_name: person.display_name } : null,
        health_rating: null,
      }
      projects.push(project)
      members.set(project.id, new Set(person ? [person.id] : []))
      return HttpResponse.json<Project>(project, { status: 201 })
    }),
    http.get(api('/projects/:id'), ({ params }) => {
      const project = projects.find((p) => p.id === params.id)
      return project ? HttpResponse.json<Project>(project) : problem(404, 'Not Found')
    }),
    http.get(api('/projects/:id/members'), ({ params }) => {
      const ids = members.get(params.id as string)
      if (!ids) return problem(404, 'Not Found')
      const items = [...ids]
        .map(personRef)
        .sort((a, b) => a.display_name.localeCompare(b.display_name))
      return HttpResponse.json<Schemas['MemberList']>({ items })
    }),
    http.post(api('/projects/:id/members'), async ({ request, params }) => {
      const project = writable(params.id, request)
      if (project instanceof Response) return project
      const { person_id } = (await request.json()) as Schemas['NewMember']
      if (!people.some((p) => p.id === person_id)) return problem(422, 'Unprocessable Content')
      const ids = members.get(project.id)!
      if (!ids.has(person_id)) {
        ids.add(person_id)
        project.version += 1
      }
      return HttpResponse.json<Project>(project)
    }),
    http.delete(api('/projects/:id/members/:personId'), ({ request, params }) => {
      const project = writable(params.id, request)
      if (project instanceof Response) return project
      const ids = members.get(project.id)!
      if (!ids.delete(params.personId as string)) return problem(404, 'Not Found')
      if (project.project_leader?.id === params.personId) project.project_leader = null
      actions.unassign(project.id, params.personId as string)
      blockers.unassign(project.id, params.personId as string)
      project.version += 1
      return HttpResponse.json<Project>(project)
    }),
    http.put(api('/projects/:id'), async ({ request, params }) => {
      const project = writable(params.id, request)
      if (project instanceof Response) return project
      if (lockedOut()) return problem(409, 'Conflict')
      const body = (await request.json()) as Project
      if (!('description' in body) || !('health_rating' in body) || !('project_leader' in body))
        return problem(422, 'Unprocessable Content')
      const leader = body.project_leader?.id ?? null
      if (leader && !members.get(project.id)!.has(leader))
        return problem(422, 'Unprocessable Content')
      project.name = body.name.trim()
      project.description = body.description ?? null
      project.project_leader = leader ? personRef(leader) : null
      const health = body.health_rating
      project.health_rating = health
        ? { rating: health.rating, comment: health.comment?.trim() || null }
        : null
      project.version += 1
      return HttpResponse.json<Project>(project)
    }),
    http.get(api('/projects/:id'), ({ params }) => {
      const project = projects.find((p) => p.id === params.id)
      return project ? HttpResponse.json<Project>(project) : problem(404, 'Not Found')
    }),
    http.patch(api('/projects/:id'), async ({ request, params }) => {
      const project = projects.find((p) => p.id === params.id)
      if (!project) return problem(404, 'Not Found')
      if (request.headers.get('Content-Type') !== 'application/merge-patch+json')
        return problem(415, 'Unsupported Media Type')
      if (request.headers.get('If-Match') !== `"${project.version}"`)
        return problem(412, 'Precondition Failed')
      const { archived } = (await request.json()) as Schemas['ProjectPatch']
      if (project.archived !== archived) {
        project.archived = archived
        project.version += 1
      }
      return HttpResponse.json<Project>(project)
    }),
    http.get(api('/projects/:id/meeting-lock'), () =>
      lock ? HttpResponse.json<MeetingLock>(lock) : problem(404, 'Not Found'),
    ),
    http.post(api('/projects/:id/meeting-lock'), ({ params }) => {
      const project = projects.find((p) => p.id === params.id)
      if (!project) return problem(404, 'Not Found')
      if (project.project_leader?.id !== acting) return problem(403, 'Forbidden')
      if (lockedOut()) return problem(409, 'Conflict')
      lock = {
        leader: personRef(acting!),
        acquired_at: new Date().toISOString(),
        live: true,
        held_by_you: true,
      }
      return HttpResponse.json<MeetingLock>(lock)
    }),
    // Another session taking over (setLock with held_by_you false) makes these 409.
    http.put(api('/projects/:id/meeting-lock'), () => {
      if (!lock) return problem(404, 'Not Found')
      if (!lock.held_by_you) return problem(409, 'Conflict')
      heartbeats++
      lock = { ...lock, live: true }
      return HttpResponse.json<MeetingLock>(lock)
    }),
    http.delete(api('/projects/:id/meeting-lock'), () => {
      if (!lock) return problem(404, 'Not Found')
      if (!lock.held_by_you) return problem(409, 'Conflict')
      lock = null
      return new HttpResponse(null, { status: 204 })
    }),
    http.post(api('/meetings'), async ({ request }) => {
      const body = (await request.json()) as Schemas['NewMeetingBody']
      if (!lock?.held_by_you) return problem(409, 'Conflict')
      const project = projects.find((p) => p.id === body.project_id)
      if (project?.project_leader?.id !== acting) return problem(403, 'Forbidden')
      const leader = lock.leader
      const ids = [leader.id, ...body.attendee_ids.filter((id) => id !== leader.id)]
      const meeting: MeetingSummary = {
        id: `meeting-${++n}`,
        project_id: body.project_id,
        leader,
        attendees: ids.map(personRef),
        started_at: lock.acquired_at,
        ended_at: new Date().toISOString(),
        version: 1,
      }
      meetings.push(meeting)
      created.push(body)
      lock = null
      return HttpResponse.json({ ...meeting, minutes: {} }, { status: 201 })
    }),
    http.get(api('/meetings'), ({ request }) => {
      const projectId = new URL(request.url).searchParams.get('project_id')
      const items = meetings
        .filter((m) => m.project_id === projectId)
        .sort((a, b) => b.started_at.localeCompare(a.started_at))
      return HttpResponse.json<Schemas['MeetingPage']>({
        items,
        limit: 100,
        offset: 0,
        total: items.length,
      })
    }),
    // Stand-in Minutes: just enough to tell the renderings apart.
    http.get(api('/meetings/:id/minutes'), ({ request, params }) => {
      if (!meetings.some((m) => m.id === params.id)) return problem(404, 'Not Found')
      const accept = request.headers.get('Accept')
      if (accept === 'text/html')
        return HttpResponse.html(`<p>Formatted Minutes of ${params.id}</p>`)
      if (accept === 'text/plain') return HttpResponse.text(`Plain Minutes of ${params.id}`)
      if (accept === 'application/json')
        return HttpResponse.json({
          schema: 1,
          project_name: `Formatted Minutes of ${params.id}`,
          start: '2026-10-21T09:00:00Z',
          end: '2026-10-21T10:15:00Z',
          duration_minutes: 75,
          leader: 'Ada',
          attendees: ['Ada', 'Bob'],
          actions: {
            new: [
              { reference: 'A-1', title: '<b>Ship</b>', state: ['Due 31 Oct'], comments: ['Soon'] },
            ],
            updated: [{ reference: 'A-2', title: 'Test', changes: ['Closed'], comments: [] }],
          },
          meeting_comments: ['Good meeting'],
        })
      return problem(406, 'Not Acceptable')
    }),
    http.get(api('/session'), () => {
      const person = people.find((p) => p.id === acting)
      return person
        ? HttpResponse.json<Schemas['Session']>({ person })
        : problem(401, 'Unauthorized')
    }),
    http.post(api('/session'), async ({ request }) => {
      const { person_id } = (await request.json()) as Schemas['NewSession']
      const person = people.find((p) => p.id === person_id)
      if (!person) return problem(422, 'Unprocessable Content')
      acting = person.id
      return HttpResponse.json<Schemas['Session']>({ person })
    }),
  ]
  const risks = fakeRisks(projects, initialRisks)
  handlers.push(...risks.handlers)
  const actions = fakeActions(projects, members, personRef, initialActions, (id) =>
    blockers.blockedBy(id),
  )
  handlers.push(...actions.handlers)
  const blockers = fakeBlockers(projects, members, personRef, actions.actions, initialBlockers)
  handlers.push(...blockers.handlers)
  const comments = fakeComments(projects, members, personRef, () => acting, initialComments)
  handlers.push(...comments.handlers)
  return {
    handlers,
    people,
    projects,
    members,
    risks: risks.risks,
    actions: actions.actions,
    blockers: blockers.blockers,
    comments: comments.comments,
    acting: () => acting,
    meetings,
    created,
    heartbeats: () => heartbeats,
    lock: () => lock,
    setLock: (next: MeetingLock | null) => (lock = next),
  }
}

export const server = setupServer()
