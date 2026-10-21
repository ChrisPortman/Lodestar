<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Project } from '../api/client'
import type { MeetingSummary } from '../api/meetings'
import { clockTime } from '../meetingLock'
import {
  meetingLength,
  meetingStart,
  useMeetingLock,
  useMeetings,
  useStartMeeting,
} from '../meetings'
import { useSession } from '../session'
import MinutesReader from './MinutesReader.vue'

const props = defineProps<{ project: Project }>()
const projectId = computed(() => props.project.id)
const session = useSession()
const lock = useMeetingLock(projectId)
const meetings = useMeetings(projectId)
const start = useStartMeeting(projectId)

const isLeader = computed(
  () =>
    !!props.project.project_leader &&
    props.project.project_leader.id === session.data.value?.person.id,
)
const runningHere = computed(() => !!lock.data.value?.held_by_you)
const runningElsewhere = computed(() => {
  const l = lock.data.value
  return l && l.live && !l.held_by_you ? l : null
})

const reading = ref<MeetingSummary | null>(null)
</script>

<template>
  <div class="mt-4 grid gap-4">
    <p v-if="runningHere" role="status" class="rounded bg-accent-soft p-3 text-sm">
      Your Meeting is running.
    </p>
    <p
      v-else-if="runningElsewhere"
      role="status"
      class="rounded bg-st-amber-bg p-3 text-sm text-st-amber-fg"
    >
      {{ runningElsewhere.leader.display_name }} is running a Meeting, since
      {{ clockTime(runningElsewhere.acquired_at) }}.
    </p>
    <button
      v-else-if="isLeader"
      type="button"
      class="btn btn-primary btn-lg"
      :disabled="start.isPending.value || lock.isPending.value"
      @click="start.mutate()"
    >
      Start a Meeting now
    </button>
    <p v-else class="text-sm text-muted">Only a Project Leader can start a Meeting.</p>
    <p v-if="start.error.value" role="alert" class="text-danger-text">
      Couldn't start the Meeting: {{ start.error.value.message }}
    </p>

    <section aria-labelledby="past-meetings-title">
      <h3 id="past-meetings-title" class="font-semibold">Past Meetings</h3>
      <p v-if="meetings.error.value" role="alert" class="mt-2 text-danger-text">
        Couldn't load Meetings: {{ meetings.error.value.message }}
      </p>
      <p v-else-if="meetings.data.value?.length === 0" class="mt-2 text-sm text-muted">
        No Meetings yet.
      </p>
      <ul v-else aria-label="Past Meetings" class="mt-2 grid gap-1">
        <li v-for="m in meetings.data.value" :key="m.id">
          <button
            type="button"
            class="w-full rounded px-2 py-1 text-left hover:bg-hover"
            @click="reading = m"
          >
            <span class="block">{{ meetingStart(m) }}</span>
            <span class="block text-xs text-muted">
              {{ meetingLength(m) }} min · led by {{ m.leader.display_name }} ·
              {{ m.attendees.length }} attendees
            </span>
          </button>
        </li>
      </ul>
    </section>
    <MinutesReader v-if="reading" :meeting="reading" @close="reading = null" />
  </div>
</template>
