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

export function useMembers(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => [...projectsKey, 'members', projectId.value]),
    queryFn: () => listMembers(projectId.value),
  })
}

// Member changes bump the Project's version, so every Project query is refreshed.
function useMemberMutation<T>(mutationFn: (input: T) => Promise<Project>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: projectsKey }),
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
