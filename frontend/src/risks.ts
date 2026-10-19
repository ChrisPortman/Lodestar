import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import {
  createRisk,
  listRisks,
  setRiskClosed,
  updateRisk,
  type Risk,
  type RiskFields,
} from './api/risks'

export const risksKey = ['risks'] as const

export function useRisks(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => [...risksKey, projectId.value]),
    queryFn: () => listRisks(projectId.value),
  })
}

/** Refetches on success and failure alike, so a 412 shows the latest state. */
function useRiskMutation<T>(mutationFn: (input: T) => Promise<Risk>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSettled: () => queryClient.invalidateQueries({ queryKey: risksKey }),
  })
}

export function useCreateRisk() {
  return useRiskMutation(({ projectId, fields }: { projectId: string; fields: RiskFields }) =>
    createRisk(projectId, fields),
  )
}

export function useUpdateRisk() {
  return useRiskMutation(({ risk, fields }: { risk: Risk; fields: Partial<RiskFields> }) =>
    updateRisk(risk, fields),
  )
}

export function useSetRiskClosed() {
  return useRiskMutation(({ risk, closed }: { risk: Risk; closed: boolean }) =>
    setRiskClosed(risk, closed),
  )
}

export type Severity = 'low' | 'medium' | 'high'

/** How severe a score is: 15+ high, 8+ medium, otherwise low (meeting view prototype). */
export function severity(score: number): Severity {
  return score >= 15 ? 'high' : score >= 8 ? 'medium' : 'low'
}
