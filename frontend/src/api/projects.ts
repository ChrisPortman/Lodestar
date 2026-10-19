import { client, ifMatch, unwrap, type Project } from './client'
import type { components } from './schema'

export type HealthRating = components['schemas']['HealthRating']

export async function getProject(id: string): Promise<Project> {
  return unwrap(await client.GET('/api/v1/projects/{id}', { params: { path: { id } } }))
}

export async function listProjects(includeArchived: boolean): Promise<Project[]> {
  const page = unwrap(
    await client.GET('/api/v1/projects', {
      params: { query: { sort: 'name', limit: 1000, include_archived: includeArchived } },
    }),
  )
  return page.items
}

/** An unused Project Code for `name`, or `null` if it has too few letters. */
export async function suggestProjectCode(name: string): Promise<string | null> {
  const suggestion = unwrap(
    await client.GET('/api/v1/projects/code_suggestion', { params: { query: { name } } }),
  )
  return suggestion.project_code ?? null
}

export interface NewProject {
  name: string
  description: string | null
  projectCode: string
}

export async function createProject(input: NewProject): Promise<Project> {
  return unwrap(
    await client.POST('/api/v1/projects', {
      body: {
        name: input.name,
        description: input.description,
        project_code: input.projectCode,
      } as Project,
    }),
  )
}

export async function updateProject(
  project: Project,
  name: string,
  description: string | null,
): Promise<Project> {
  return unwrap(
    await client.PUT('/api/v1/projects/{id}', {
      params: { path: { id: project.id }, header: { 'if-match': ifMatch(project.version) } },
      body: { ...project, name, description },
    }),
  )
}

/** Sets the Health Rating (`null` for not rated), keeping every other field. */
export async function setHealthRating(
  project: Project,
  healthRating: HealthRating | null,
): Promise<Project> {
  return unwrap(
    await client.PUT('/api/v1/projects/{id}', {
      params: { path: { id: project.id }, header: { 'if-match': ifMatch(project.version) } },
      body: { ...project, health_rating: healthRating },
    }),
  )
}

export async function setArchived(project: Project, archived: boolean): Promise<Project> {
  return unwrap(
    await client.PATCH('/api/v1/projects/{id}', {
      params: { path: { id: project.id }, header: { 'if-match': ifMatch(project.version) } },
      headers: { 'Content-Type': 'application/merge-patch+json' },
      body: { archived },
    }),
  )
}
