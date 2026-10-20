// Seeds data through the public API, as a client separate from the browser.
import { expect, type APIRequestContext, type APIResponse, type Page } from '@playwright/test'

async function ok<T>(response: APIResponse): Promise<T> {
  expect(response.ok(), `${response.status()} ${await response.text()}`).toBe(true)
  return (await response.json()) as T
}

export interface Person {
  id: string
  display_name: string
}

export interface Project {
  id: string
  name: string
  project_code: string
}

export interface Action {
  id: string
  reference: string
  version: number
  [field: string]: unknown
}

export async function seedPerson(request: APIRequestContext, displayName: string) {
  return ok<Person>(await request.post('/api/v1/people', { data: { display_name: displayName } }))
}

let codes = 0

/** A Person acting through `request`, and a Project they created. */
export async function seedProject(request: APIRequestContext, name: string) {
  const person = await seedPerson(request, `${name} Leader`)
  await ok(await request.post('/api/v1/session', { data: { person_id: person.id } }))
  const project_code = `E${Date.now().toString(36).toUpperCase().slice(-6)}${codes++}`
  const project = await ok<Project>(
    await request.post('/api/v1/projects', { data: { name, project_code } }),
  )
  return { person, project }
}

export async function seedAction(
  request: APIRequestContext,
  projectId: string,
  description: string,
) {
  return ok<Action>(
    await request.post('/api/v1/actions', {
      data: { project_id: projectId, description, assignee: null, due_date: null },
    }),
  )
}

export async function updateAction(
  request: APIRequestContext,
  action: Action,
  fields: Partial<Action>,
) {
  return ok<Action>(
    await request.put(`/api/v1/actions/${action.id}`, {
      headers: { 'If-Match': `"${action.version}"` },
      data: { ...action, ...fields },
    }),
  )
}

export async function seedRisk(
  request: APIRequestContext,
  projectId: string,
  description: string,
  likelihood: number,
  impact: number,
) {
  return ok<{ id: string }>(
    await request.post('/api/v1/risks', {
      data: { project_id: projectId, description, likelihood, impact },
    }),
  )
}

/** A Blocker with no one accountable, blocking `actionIds`. */
export async function seedBlocker(
  request: APIRequestContext,
  projectId: string,
  description: string,
  actionIds: string[] = [],
) {
  let blocker = await ok<{ id: string; version: number }>(
    await request.post('/api/v1/blockers', {
      data: { project_id: projectId, description, accountable: null },
    }),
  )
  for (const action_id of actionIds)
    blocker = await ok(
      await request.post(`/api/v1/blockers/${blocker.id}/blocked_actions`, {
        headers: { 'If-Match': `"${blocker.version}"` },
        data: { action_id },
      }),
    )
  return blocker
}

export async function setHealthRating(
  request: APIRequestContext,
  projectId: string,
  rating: 'red' | 'amber' | 'green',
  comment: string | null = null,
) {
  const project = await ok<Project & { version: number }>(
    await request.get(`/api/v1/projects/${projectId}`),
  )
  return ok(
    await request.put(`/api/v1/projects/${projectId}`, {
      headers: { 'If-Match': `"${project.version}"` },
      data: { ...project, health_rating: { rating, comment } },
    }),
  )
}

/** Gives the browser a session as `person`, skipping the picker. */
export async function actAs(page: Page, person: Person) {
  await ok(await page.request.post('/api/v1/session', { data: { person_id: person.id } }))
}

/**
 * Fills a Project with every status Project on a Page styles: a Red Health
 * Rating, an overdue Action, a Blocker and a high-scoring Risk.
 */
export async function seedReportItems(request: APIRequestContext, project: Project) {
  const current = await ok<Project & { version: number }>(
    await request.get(`/api/v1/projects/${project.id}`),
  )
  await ok(
    await request.put(`/api/v1/projects/${project.id}`, {
      headers: { 'If-Match': `"${current.version}"` },
      data: { ...current, health_rating: { rating: 'red', comment: 'Venue fell through' } },
    }),
  )
  await ok(
    await request.post('/api/v1/actions', {
      data: {
        project_id: project.id,
        description: 'Confirm caterer',
        assignee: null,
        due_date: '2001-01-01',
      },
    }),
  )
  await ok(
    await request.post('/api/v1/blockers', {
      data: { project_id: project.id, description: 'No budget', accountable: null },
    }),
  )
  await ok(
    await request.post('/api/v1/risks', {
      data: { project_id: project.id, description: 'Storm', likelihood: 4, impact: 5 },
    }),
  )
}
