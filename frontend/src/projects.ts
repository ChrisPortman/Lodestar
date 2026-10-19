import { computed, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import {
  createProject,
  listProjects,
  setArchived,
  updateProject,
  type NewProject,
} from './api/projects'
import type { Project } from './api/client'

export const projectsKey = ['projects'] as const

export function useProjects(includeArchived: Ref<boolean>) {
  return useQuery({
    queryKey: computed(() => [...projectsKey, { includeArchived: includeArchived.value }]),
    queryFn: () => listProjects(includeArchived.value),
  })
}

function useProjectMutation<T>(mutationFn: (input: T) => Promise<Project>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: projectsKey }),
  })
}

export function useCreateProject() {
  return useProjectMutation((input: NewProject) => createProject(input))
}

export function useUpdateProject() {
  return useProjectMutation(
    ({
      project,
      name,
      description,
    }: {
      project: Project
      name: string
      description: string | null
    }) => updateProject(project, name, description),
  )
}

export function useSetArchived() {
  return useProjectMutation(({ project, archived }: { project: Project; archived: boolean }) =>
    setArchived(project, archived),
  )
}
