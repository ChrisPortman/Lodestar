import { client, ifMatch, unwrap, type Project } from './client'
import type { components } from './schema'

export type PersonRef = components['schemas']['PersonRef']

/** A Project's Members, ordered by display name. */
export async function listMembers(projectId: string): Promise<PersonRef[]> {
  const list = unwrap(
    await client.GET('/api/v1/projects/{id}/members', { params: { path: { id: projectId } } }),
  )
  return list.items
}

export async function addMember(project: Project, personId: string): Promise<Project> {
  return unwrap(
    await client.POST('/api/v1/projects/{id}/members', {
      params: { path: { id: project.id }, header: { 'if-match': ifMatch(project.version) } },
      body: { person_id: personId },
    }),
  )
}

export async function removeMember(project: Project, personId: string): Promise<Project> {
  return unwrap(
    await client.DELETE('/api/v1/projects/{id}/members/{person_id}', {
      params: {
        path: { id: project.id, person_id: personId },
        header: { 'if-match': ifMatch(project.version) },
      },
    }),
  )
}

/** Makes a Member the Project Leader, keeping every other field. */
export async function setProjectLeader(project: Project, leader: PersonRef): Promise<Project> {
  return unwrap(
    await client.PUT('/api/v1/projects/{id}', {
      params: { path: { id: project.id }, header: { 'if-match': ifMatch(project.version) } },
      body: { ...project, project_leader: leader },
    }),
  )
}
