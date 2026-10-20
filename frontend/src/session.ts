import { useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/vue-query'
import { createPerson, getSession, listPeople, renamePerson, startSession } from './api/people'
import type { Person, Session } from './api/client'

export const sessionKey = ['session'] as const
export const peopleKey = ['people'] as const

export const sessionQuery = { queryKey: sessionKey, queryFn: getSession, staleTime: Infinity }

export function useSession() {
  return useQuery(sessionQuery)
}

export function fetchSession(queryClient: QueryClient): Promise<Session | null> {
  return queryClient.fetchQuery(sessionQuery)
}

export function usePeople() {
  return useQuery({ queryKey: peopleKey, queryFn: listPeople })
}

/** Makes an existing Person the Acting Person. */
export function useStartSession() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: startSession,
    onSuccess: (session) => queryClient.setQueryData(sessionKey, session),
  })
}

/** Creates a Person, who becomes the Acting Person if there was none. */
export function useCreatePerson() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: createPerson,
    onSuccess: (person) => {
      if (!queryClient.getQueryData<Session | null>(sessionKey))
        queryClient.setQueryData<Session>(sessionKey, { person })
      return queryClient.invalidateQueries({ queryKey: peopleKey })
    },
  })
}

export function useRenamePerson() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ person, displayName }: { person: Person; displayName: string }) =>
      renamePerson(person, displayName),
    onSuccess: (person) => {
      const session = queryClient.getQueryData<Session | null>(sessionKey)
      if (session?.person.id === person.id) queryClient.setQueryData(sessionKey, { person })
      return queryClient.invalidateQueries({ queryKey: peopleKey })
    },
  })
}

/** The option value behind "New Person…" in a Person dropdown (#59). */
export const NEW_PERSON = '__new_person__'
