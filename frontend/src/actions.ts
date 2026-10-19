import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import {
  createAction,
  listActions,
  setActionClosed,
  updateAction,
  type Action,
  type ActionFields,
} from './api/actions'

export const actionsKey = ['actions'] as const

export function useActions(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => [...actionsKey, projectId.value]),
    queryFn: () => listActions(projectId.value),
  })
}

/** Refetches on success and failure alike, so a 412 shows the latest state. */
function useActionMutation<T>(mutationFn: (input: T) => Promise<Action>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSettled: () => queryClient.invalidateQueries({ queryKey: actionsKey }),
  })
}

export function useCreateAction() {
  return useActionMutation(({ projectId, fields }: { projectId: string; fields: ActionFields }) =>
    createAction(projectId, fields),
  )
}

export function useUpdateAction() {
  return useActionMutation(
    ({ action, fields }: { action: Action; fields: Partial<ActionFields> }) =>
      updateAction(action, fields),
  )
}

export function useSetActionClosed() {
  return useActionMutation(({ action, closed }: { action: Action; closed: boolean }) =>
    setActionClosed(action, closed),
  )
}

/** Today as `YYYY-MM-DD` in local time. */
export function today(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
}

/** An open Action past its due date (FR-33). */
export function isOverdue(action: Action, on = today()): boolean {
  return !action.closed && action.due_date != null && action.due_date < on
}
