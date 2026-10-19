import createClient from 'openapi-fetch'
import type { components, paths } from './schema'

export type Problem = components['schemas']['Problem']
export type Person = components['schemas']['Person']
export type Session = components['schemas']['Session']

/** A failed API call, carrying the server's `problem+json` body. */
export class ApiError extends Error {
  readonly status: number
  readonly problem: Problem

  constructor(problem: Problem) {
    super(problem.detail ?? problem.title)
    this.name = 'ApiError'
    this.status = problem.status
    this.problem = problem
  }
}

export const client = createClient<paths>({
  baseUrl: globalThis.location?.origin ?? '',
  credentials: 'same-origin',
  // Resolve fetch per call so test interceptors installed later are honoured.
  fetch: (request) => globalThis.fetch(request),
})

/** The `If-Match` value for a resource at `version` (the server's ETag is `"<version>"`). */
export function ifMatch(version: number): string {
  return `"${version}"`
}

/** Returns `data`, or throws an {@link ApiError} built from the error response. */
export function unwrap<T>(result: { data?: T; error?: unknown; response: Response }): T {
  if (result.response.ok) return result.data as T
  const error = result.error
  const problem: Problem =
    error && typeof error === 'object' && 'status' in error && 'title' in error
      ? (error as Problem)
      : {
          type: 'about:blank',
          status: result.response.status,
          title: result.response.statusText || 'Request failed',
        }
  throw new ApiError(problem)
}
