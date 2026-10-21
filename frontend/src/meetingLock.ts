// The Meeting in progress banner (#80): another session's live Meeting Lock
// blocks editing on the Project page until it is released or goes stale.
import { computed, inject, onScopeDispose, provide, watch, type ComputedRef, type Ref } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { ApiError } from './api/client'
import { getMeetingLock, type MeetingLock } from './api/meetingLock'

/** How often the Project page checks for another session's Meeting. */
export const LOCK_POLL_MS = 30_000

export const meetingLockKey = (projectId: string) => ['meeting-lock', projectId] as const

const blockingKey = Symbol('blocking Meeting Lock')

/**
 * Polls a Project's Meeting Lock and returns the lock that blocks this
 * session (live and held by another session), or `null`. A 409 from any write
 * refetches it at once, and the page's data is refetched when the blocking
 * lock goes away. Provides the result to {@link useWriteError}.
 */
export function useBlockingLock(projectId: Ref<string>): ComputedRef<MeetingLock | null> {
  const queryClient = useQueryClient()
  const query = useQuery({
    queryKey: computed(() => meetingLockKey(projectId.value)),
    queryFn: () => getMeetingLock(projectId.value),
    refetchInterval: LOCK_POLL_MS,
  })
  const blocking = computed(() => {
    const lock = query.data.value
    return lock && lock.live && !lock.held_by_you ? lock : null
  })

  // A write can race a Meeting starting; its 409 shows the banner straight away.
  const unsubscribe = queryClient.getMutationCache().subscribe((event) => {
    if (event.type === 'updated' && event.action.type === 'error' && isConflict(event.action.error))
      void query.refetch()
  })
  onScopeDispose(unsubscribe)

  watch(blocking, (now, before) => {
    if (before && !now) void queryClient.invalidateQueries()
  })

  provide(blockingKey, blocking)
  return blocking
}

function isConflict(error: unknown): boolean {
  return error instanceof ApiError && error.status === 409
}

/**
 * The first error among a component's writes (`mutations`), to show next to
 * them. A 409 while a Meeting blocks this session is left out: the Meeting in
 * progress banner explains that instead of a generic error.
 */
export function useWriteError<E>(...mutations: { error: Ref<E | null> }[]): ComputedRef<E | null> {
  const blocking = inject<Ref<MeetingLock | null> | null>(blockingKey, null)
  return computed(() => {
    const error = mutations.map((m) => m.error.value).find((e) => e != null) ?? null
    return blocking?.value && isConflict(error) ? null : error
  })
}

/** "HH:MM" in local time. */
export function clockTime(iso: string): string {
  const at = new Date(iso)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(at.getHours())}:${pad(at.getMinutes())}`
}
