import { http, HttpResponse } from 'msw'
import type { components } from '../api/schema'
import { api, problem } from './server'

type Schemas = components['schemas']
export type Comment = Schemas['Comment']
type Mutable<T> = { -readonly [K in keyof T]: T[K] }

/** The `/comments` endpoints of the fake backend, over the given Projects. */
export function fakeComments(
  projects: Schemas['Project'][],
  members: Map<string, Set<string>>,
  personRef: (id: string) => Schemas['PersonRef'],
  acting: () => string | null,
  initial: Comment[],
) {
  const comments = initial.map((c) => ({ ...c }) as Mutable<Comment>)
  let n = 0

  /** Who may change Comments in `projectId`, or the problem that stops them. */
  function permitted(projectId: string): string | Response {
    const person = acting()
    if (!person) return problem(401, 'Unauthorized')
    if (projects.find((p) => p.id === projectId)?.archived) return problem(409, 'Conflict')
    if (!members.get(projectId)?.has(person)) return problem(403, 'Forbidden')
    return person
  }

  // Finds the Acting Person's own Comment to change with `If-Match`, or the problem to return.
  function writable(request: Request, id: unknown): Mutable<Comment> | Response {
    const comment = comments.find((c) => c.id === id)
    if (!comment) return problem(404, 'Not Found')
    const person = permitted(comment.project_id)
    if (person instanceof Response) return person
    if (comment.author.id !== person) return problem(403, 'Forbidden')
    if (request.headers.get('If-Match') !== `"${comment.version}"`)
      return problem(412, 'Precondition Failed')
    if (comment.deleted) return problem(409, 'Conflict')
    return comment
  }

  const handlers = [
    http.get(api('/comments'), ({ request }) => {
      const query = new URL(request.url).searchParams
      const limit = Number(query.get('limit') ?? 50)
      const offset = Number(query.get('offset') ?? 0)
      const all = comments.filter((c) => c.project_id === query.get('project_id'))
      return HttpResponse.json<Schemas['CommentPage']>({
        items: all.slice(offset, offset + limit),
        limit,
        offset,
        total: all.length,
      })
    }),
    http.post(api('/comments'), async ({ request }) => {
      const body = (await request.json()) as Comment
      const person = permitted(body.project_id)
      if (person instanceof Response) return person
      if (!body.body.trim()) return problem(422, 'Unprocessable Content')
      const comment: Mutable<Comment> = {
        id: `comment-${++n}`,
        project_id: body.project_id,
        target_type: body.target_type,
        target_id: body.target_id,
        author: personRef(person),
        body: body.body,
        created_at: new Date().toISOString(),
        edited_at: null,
        deleted: false,
        version: 1,
      }
      comments.push(comment)
      return HttpResponse.json<Comment>(comment, { status: 201 })
    }),
    http.put(api('/comments/:id'), async ({ request, params }) => {
      const comment = writable(request, params.id)
      if (comment instanceof Response) return comment
      const body = (await request.json()) as Comment
      if (!body.body.trim()) return problem(422, 'Unprocessable Content')
      comment.body = body.body
      comment.edited_at = new Date().toISOString()
      comment.version += 1
      return HttpResponse.json<Comment>(comment)
    }),
    http.patch(api('/comments/:id'), async ({ request, params }) => {
      if (request.headers.get('Content-Type') !== 'application/merge-patch+json')
        return problem(415, 'Unsupported Media Type')
      const comment = writable(request, params.id)
      if (comment instanceof Response) return comment
      const { deleted } = (await request.json()) as Schemas['CommentPatch']
      if (!deleted) return problem(422, 'Unprocessable Content')
      comment.deleted = true
      comment.body = ''
      comment.version += 1
      return HttpResponse.json<Comment>(comment)
    }),
  ]
  return { handlers, comments }
}
