import { client, ifMatch, unwrap } from './client'
import type { components } from './schema'

export type Risk = components['schemas']['Risk']

/** Every Risk in a Project, closed ones included, in Reference order. */
export async function listRisks(projectId: string): Promise<Risk[]> {
  const page = unwrap(
    await client.GET('/api/v1/risks', {
      params: {
        query: { project_id: projectId, state: 'all', sort: 'reference', limit: 1000 },
      },
    }),
  )
  return page.items
}

export interface RiskFields {
  description: string
  likelihood: number
  impact: number
}

export async function createRisk(projectId: string, fields: RiskFields): Promise<Risk> {
  return unwrap(
    await client.POST('/api/v1/risks', {
      body: { project_id: projectId, ...fields } as Risk,
    }),
  )
}

/** Replaces the editable fields, keeping the rest as read. */
export async function updateRisk(risk: Risk, fields: Partial<RiskFields>): Promise<Risk> {
  return unwrap(
    await client.PUT('/api/v1/risks/{id}', {
      params: { path: { id: risk.id }, header: { 'if-match': ifMatch(risk.version) } },
      body: { ...risk, ...fields },
    }),
  )
}

export async function setRiskClosed(risk: Risk, closed: boolean): Promise<Risk> {
  return unwrap(
    await client.PATCH('/api/v1/risks/{id}', {
      params: { path: { id: risk.id }, header: { 'if-match': ifMatch(risk.version) } },
      headers: { 'Content-Type': 'application/merge-patch+json' },
      body: { closed },
    }),
  )
}
