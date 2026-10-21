// The running Meeting (#85): what the leader's page keeps while their
// Meeting runs, the heartbeat that keeps its lock live, and ending or
// cancelling it. Attendees, Comments and the start survive a reload in
// localStorage, keyed by Project.
import {
  computed,
  inject,
  onScopeDispose,
  provide,
  reactive,
  ref,
  watch,
  type InjectionKey,
  type Ref,
} from 'vue'
import { useQueryClient } from '@tanstack/vue-query'
import { ApiError } from './api/client'
import { cancelMeetingLock, heartbeatMeetingLock } from './api/meetingLock'
import { createMeeting, type MeetingSummary } from './api/meetings'
import { meetingLockKey } from './meetingLock'
import { meetingsKey, useMeetingLock } from './meetings'

/** How often the leader's page heartbeats its Meeting Lock. */
export const HEARTBEAT_MS = 30_000

export interface MeetingComment {
  body: string
  at: string
}

interface Saved {
  startedAt: string
  attendeeIds: string[]
  comments: MeetingComment[]
}

const storageKey = (projectId: string) => `lodestar.meeting.${projectId}`

function load(projectId: string): Saved | null {
  try {
    const raw = localStorage.getItem(storageKey(projectId))
    return raw ? (JSON.parse(raw) as Saved) : null
  } catch {
    return null
  }
}

function save(projectId: string, saved: Saved) {
  try {
    localStorage.setItem(storageKey(projectId), JSON.stringify(saved))
  } catch {
    // Without storage the Meeting still runs; it just can't be resumed.
  }
}

function forget(projectId: string) {
  try {
    localStorage.removeItem(storageKey(projectId))
  } catch {
    // Nothing to forget.
  }
}

function isConflict(error: unknown): boolean {
  return error instanceof ApiError && error.status === 409
}

/** "4:05" under an hour, then "1:04:05". */
export function elapsed(fromIso: string, now: number): string {
  const total = Math.max(0, Math.floor((now - Date.parse(fromIso)) / 1000))
  const pad = (n: number) => String(n).padStart(2, '0')
  const [h, m, s] = [Math.floor(total / 3600), Math.floor(total / 60) % 60, total % 60]
  return h ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`
}

function useRunningMeetingState(projectId: Ref<string>) {
  const queryClient = useQueryClient()
  const lock = useMeetingLock(projectId)
  const heldByYou = computed(() => !!lock.data.value?.held_by_you)
  const leaderId = computed(() => lock.data.value?.leader.id ?? null)

  const state = reactive<Saved>({ startedAt: '', attendeeIds: [], comments: [] })
  /** Started or carried on in this page; otherwise a reload must ask first. */
  const confirmed = ref(false)
  /** Someone took over a stale lock; the Comments stay visible to copy. */
  const lost = ref(false)
  const finishing = ref(false)
  const panelOpen = ref(false)
  /** The Minutes just created, for the reader. */
  const created = ref<MeetingSummary | null>(null)
  const now = ref(Date.now())

  const running = computed(() => heldByYou.value || lost.value)
  const needsResume = computed(() => heldByYou.value && !confirmed.value && !lost.value)

  function reset(saved: Saved | null) {
    state.startedAt = saved?.startedAt ?? lock.data.value?.acquired_at ?? new Date().toISOString()
    state.attendeeIds = saved?.attendeeIds ?? []
    state.comments = saved?.comments ?? []
  }
  const persist = () => save(projectId.value, { ...state })

  // A reload finds this session's lock: offer to carry on with what was saved.
  watch(
    heldByYou,
    (held, before) => {
      if (held && !confirmed.value) {
        reset(load(projectId.value))
        panelOpen.value = true
      }
      if (before && !held && !finishing.value && confirmed.value) lost.value = true
    },
    { immediate: true },
  )

  let heartbeat: ReturnType<typeof setInterval> | undefined
  let clock: ReturnType<typeof setInterval> | undefined
  watch(
    () => heldByYou.value && confirmed.value,
    (beating) => {
      clearInterval(heartbeat)
      clearInterval(clock)
      if (!beating) return
      heartbeat = setInterval(async () => {
        try {
          queryClient.setQueryData(
            meetingLockKey(projectId.value),
            await heartbeatMeetingLock(projectId.value),
          )
        } catch (e) {
          if (isConflict(e)) loseIt()
        }
      }, HEARTBEAT_MS)
      clock = setInterval(() => (now.value = Date.now()), 1000)
    },
    { immediate: true },
  )
  onScopeDispose(() => {
    clearInterval(heartbeat)
    clearInterval(clock)
  })

  function loseIt() {
    lost.value = true
    void queryClient.invalidateQueries({ queryKey: meetingLockKey(projectId.value) })
  }

  function finish() {
    forget(projectId.value)
    confirmed.value = false
    panelOpen.value = false
    queryClient.setQueryData(meetingLockKey(projectId.value), null)
  }

  return {
    leaderId,
    running,
    needsResume,
    lost,
    panelOpen,
    created,
    state,
    elapsed: computed(() => elapsed(state.startedAt, now.value)),
    /** Called once this session has acquired the lock. */
    started(acquiredAt: string) {
      reset({ startedAt: acquiredAt, attendeeIds: [], comments: [] })
      persist()
      confirmed.value = true
      now.value = Date.now()
      panelOpen.value = true
    },
    carryOn() {
      confirmed.value = true
      persist()
    },
    isHere: (id: string) => id === leaderId.value || state.attendeeIds.includes(id),
    toggleHere(id: string) {
      if (id === leaderId.value) return
      state.attendeeIds = state.attendeeIds.includes(id)
        ? state.attendeeIds.filter((a) => a !== id)
        : [...state.attendeeIds, id]
      persist()
    },
    addComment(body: string) {
      state.comments.push({ body, at: new Date().toISOString() })
      persist()
    },
    /** Creates the Meeting and its Minutes; a 409 means the lock was lost. */
    async end() {
      finishing.value = true
      try {
        const meeting = await createMeeting({
          project_id: projectId.value,
          attendee_ids: [leaderId.value!, ...state.attendeeIds],
          comments: state.comments.map(({ body }) => ({ body })),
        })
        finish()
        created.value = meeting
        void queryClient.invalidateQueries({ queryKey: meetingsKey(projectId.value) })
      } catch (e) {
        if (isConflict(e)) loseIt()
        throw e
      } finally {
        finishing.value = false
      }
    },
    /** Releases the lock and discards the Comments. */
    async cancel() {
      finishing.value = true
      try {
        await cancelMeetingLock(projectId.value)
        finish()
      } catch (e) {
        if (isConflict(e)) loseIt()
        throw e
      } finally {
        finishing.value = false
      }
    },
    /** Puts away a lost Meeting once its Comments have been copied. */
    dismiss() {
      forget(projectId.value)
      lost.value = false
      confirmed.value = false
      panelOpen.value = false
    },
  }
}

export type RunningMeeting = ReturnType<typeof useRunningMeetingState>
const key: InjectionKey<RunningMeeting> = Symbol('running Meeting')

/** Creates the Project page's running Meeting and provides it to the panels. */
export function provideRunningMeeting(projectId: Ref<string>): RunningMeeting {
  const meeting = useRunningMeetingState(projectId)
  provide(key, meeting)
  return meeting
}

export function useRunningMeeting(): RunningMeeting {
  const meeting = inject(key)
  if (!meeting) throw new Error('useRunningMeeting needs provideRunningMeeting')
  return meeting
}
