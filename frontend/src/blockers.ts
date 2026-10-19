import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import {
  createBlocker,
  linkAction,
  listBlockers,
  setBlockerCleared,
  unlinkAction,
  updateBlocker,
  type Blocker,
  type BlockerFields,
} from './api/blockers'
import { actionsKey } from './actions'

export const blockersKey = ['blockers'] as const

export function useBlockers(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => [...blockersKey, projectId.value]),
    queryFn: () => listBlockers(projectId.value),
  })
}

/**
 * Refetches on success and failure alike, so a 412 shows the latest state.
 * Actions are refetched too: they show the open Blockers blocking them.
 */
function useBlockerMutation<T>(mutationFn: (input: T) => Promise<Blocker>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: blockersKey }),
        queryClient.invalidateQueries({ queryKey: actionsKey }),
      ]),
  })
}

export function useCreateBlocker() {
  return useBlockerMutation(({ projectId, fields }: { projectId: string; fields: BlockerFields }) =>
    createBlocker(projectId, fields),
  )
}

export function useUpdateBlocker() {
  return useBlockerMutation(
    ({ blocker, fields }: { blocker: Blocker; fields: Partial<BlockerFields> }) =>
      updateBlocker(blocker, fields),
  )
}

export function useSetBlockerCleared() {
  return useBlockerMutation(({ blocker, cleared }: { blocker: Blocker; cleared: boolean }) =>
    setBlockerCleared(blocker, cleared),
  )
}

export function useLinkAction() {
  return useBlockerMutation(({ blocker, actionId }: { blocker: Blocker; actionId: string }) =>
    linkAction(blocker, actionId),
  )
}

export function useUnlinkAction() {
  return useBlockerMutation(({ blocker, actionId }: { blocker: Blocker; actionId: string }) =>
    unlinkAction(blocker, actionId),
  )
}
