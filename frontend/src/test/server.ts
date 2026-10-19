import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'
import type { components } from '../api/schema'

export type Person = components['schemas']['Person']
type Schemas = components['schemas']

export const api = (path: string) => `${location.origin}/api/v1${path}`

// Typed as a plain Response so resolvers may return either a body or a problem.
export function problem(status: number, title: string): Response {
  return HttpResponse.json<Schemas['Problem']>(
    { type: 'about:blank', status, title },
    { status, headers: { 'Content-Type': 'application/problem+json' } },
  )
}

/** An in-memory backend for the People and session endpoints. */
export function fakeBackend(initial: Person[], actingId: string | null = null) {
  const people = initial.map((p) => ({ ...p }))
  let acting = actingId
  let n = 0
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
  return { handlers, people, acting: () => acting }
}

export const server = setupServer()
