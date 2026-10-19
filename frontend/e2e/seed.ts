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

/** Changes an Action as another client would, bumping its version. */
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

/** Gives the browser a session as `person`, skipping the picker. */
export async function actAs(page: Page, person: Person) {
  await ok(await page.request.post('/api/v1/session', { data: { person_id: person.id } }))
}
