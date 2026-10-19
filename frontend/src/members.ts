import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import {
  addMember,
  listMembers,
  removeMember,
  setProjectLeader,
  type PersonRef,
} from './api/members'
import type { Project } from './api/client'
import { projectsKey } from './projects'
import { actionsKey } from './actions'

export function useMembers(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => [...projectsKey, 'members', projectId.value]),
    queryFn: () => listMembers(projectId.value),
  })
}

// Member changes bump the Project's version, so every Project query is
// refreshed, and removal unassigns open items, so those are too.
function useMemberMutation<T>(mutationFn: (input: T) => Promise<Project>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: projectsKey }),
        queryClient.invalidateQueries({ queryKey: actionsKey }),
      ]),
  })
}

export function useAddMember() {
  return useMemberMutation(({ project, personId }: { project: Project; personId: string }) =>
    addMember(project, personId),
  )
}

export function useRemoveMember() {
  return useMemberMutation(({ project, personId }: { project: Project; personId: string }) =>
    removeMember(project, personId),
  )
}

export function useSetProjectLeader() {
  return useMemberMutation(({ project, leader }: { project: Project; leader: PersonRef }) =>
    setProjectLeader(project, leader),
  )
}
