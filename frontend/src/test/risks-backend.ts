import { http, HttpResponse } from 'msw'
import type { components } from '../api/schema'
import { api, problem } from './server'

type Schemas = components['schemas']
export type Risk = Schemas['Risk']
type Mutable<T> = { -readonly [K in keyof T]: T[K] }

const inRange = (n: unknown) => Number.isInteger(n) && (n as number) >= 1 && (n as number) <= 5

/** The `/risks` endpoints of the fake backend, over the given Projects. */
export function fakeRisks(projects: Schemas['Project'][], initial: Risk[]) {
  const risks = initial.map((r) => ({ ...r }) as Mutable<Risk>)
  let n = 0

  // Finds a Risk that may be changed with `If-Match`, or the problem to return.
  function writable(request: Request, id: unknown): Mutable<Risk> | Response {
    const risk = risks.find((r) => r.id === id)
    if (!risk) return problem(404, 'Not Found')
    if (request.headers.get('If-Match') !== `"${risk.version}"`)
      return problem(412, 'Precondition Failed')
    if (projects.find((p) => p.id === risk.project_id)?.archived) return problem(409, 'Conflict')
    return risk
  }

  const handlers = [
    http.get(api('/risks'), ({ request }) => {
      const query = new URL(request.url).searchParams
      const inState = (state: string, closed: boolean) =>
        state === 'all' || (state === 'closed') === closed
      const items = risks.filter(
        (r) =>
          (!query.has('project_id') || r.project_id === query.get('project_id')) &&
          inState(query.get('state') ?? 'open', r.closed),
      )
      return HttpResponse.json<Schemas['RiskPage']>({
        items,
        limit: 1000,
        offset: 0,
        total: items.length,
      })
    }),
    http.post(api('/risks'), async ({ request }) => {
      const body = (await request.json()) as Risk
      const project = projects.find((p) => p.id === body.project_id)
      if (!project || !inRange(body.likelihood) || !inRange(body.impact))
        return problem(422, 'Unprocessable Content')
      if (project.archived) return problem(409, 'Conflict')
      const number = risks.filter((r) => r.project_id === project.id).length + 1
      const risk: Mutable<Risk> = {
        id: `risk-${++n}`,
        project_id: project.id,
        project_code: project.project_code,
        reference: `R-${String(number).padStart(3, '0')}`,
        description: body.description.trim(),
        likelihood: body.likelihood,
        impact: body.impact,
        score: body.likelihood * body.impact,
        closed: false,
        version: 1,
      }
      risks.push(risk)
      return HttpResponse.json<Risk>(risk, { status: 201 })
    }),
    http.put(api('/risks/:id'), async ({ request, params }) => {
      const risk = writable(request, params.id)
      if (risk instanceof Response) return risk
      const body = (await request.json()) as Risk
      if (!inRange(body.likelihood) || !inRange(body.impact))
        return problem(422, 'Unprocessable Content')
      risk.description = body.description.trim()
      risk.likelihood = body.likelihood
      risk.impact = body.impact
      risk.score = body.likelihood * body.impact
      risk.version += 1
      return HttpResponse.json<Risk>(risk)
    }),
    http.patch(api('/risks/:id'), async ({ request, params }) => {
      if (request.headers.get('Content-Type') !== 'application/merge-patch+json')
        return problem(415, 'Unsupported Media Type')
      const risk = writable(request, params.id)
      if (risk instanceof Response) return risk
      const { closed } = (await request.json()) as Schemas['RiskPatch']
      if (risk.closed !== closed) {
        risk.closed = closed
        risk.version += 1
      }
      return HttpResponse.json<Risk>(risk)
    }),
  ]
  return { handlers, risks }
}
