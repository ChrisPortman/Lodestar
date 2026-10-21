<script setup lang="ts">
// PROTOTYPE (#75) Variant C: start with one click from the menu; the running
// Meeting is a slim sticky bar across the top of the page with a quick-comment
// popover. Attendees are confirmed at the end, in a wrap-up dialog. Minutes
// and past Meetings open in a wide reader overlay rather than the narrow panel.
import { computed, ref } from 'vue'
import type { Project } from '../../api/client'
import { useMembers } from '../../members'
import { useSession } from '../../session'
import MinutesView from './MinutesView.prototype.vue'
import {
  addComment,
  cancel,
  end,
  fmtDate,
  fmtTime,
  minutesLong,
  removeComment,
  start,
  stub,
  useElapsed,
  type PastMeeting,
} from './meetingStub.prototype'

const props = defineProps<{ project: Project; place: 'panel' | 'page' }>()
const emit = defineEmits<{ close: [] }>()
const members = useMembers(computed(() => props.project.id))
const session = useSession()
const me = computed(() => session.data.value?.person)
const name = (id: string) => members.data.value?.find((m) => m.id === id)?.display_name ?? id

const draft = ref('')
function post() {
  addComment(draft.value)
  draft.value = ''
}
const composing = ref(false)
const wrapUp = ref(false)
const attendees = ref<string[]>([])
const elapsed = useElapsed(() => stub.running?.startedAt)

// Shared between the panel and page instances so the reader opens over the page.
const reader = readerState
function begin() {
  start([])
  emit('close')
}
function openWrapUp() {
  attendees.value = (members.data.value ?? []).map((m) => m.id)
  wrapUp.value = true
}
function finish() {
  stub.running!.attendeeIds = attendees.value
  end(name, me.value?.display_name ?? 'You')
  wrapUp.value = false
  reader.meeting = stub.justEnded
}
function read(m: PastMeeting) {
  reader.meeting = m
  emit('close')
}
</script>

<script lang="ts">
import { reactive } from 'vue'
const readerState = reactive({ meeting: null as PastMeeting | null })
</script>

<template>
  <!-- In the Project menu: a launcher and a compact history -->
  <div v-if="place === 'panel'" class="mt-4 grid gap-4">
    <div v-if="stub.role === 'other-leader-running'" class="text-sm text-st-amber-fg">
      Sam Okafor is running a Meeting.
    </div>
    <button
      v-else-if="stub.role === 'leader' && !stub.running"
      type="button"
      class="btn btn-primary btn-lg"
      @click="begin"
    >
      ● Start Meeting
    </button>
    <p v-else-if="stub.running" class="text-sm">
      Meeting running: see the bar at the top of the page.
    </p>
    <p v-else class="text-sm text-muted">Only a Project Leader can start a Meeting.</p>
    <h3 class="font-semibold">Past Meetings</h3>
    <ul class="grid gap-2">
      <li v-for="m in stub.past" :key="m.id" class="rounded border border-border p-2">
        <div class="flex items-baseline gap-2">
          <span class="flex-1 font-medium">{{ fmtDate(m.startedAt) }}</span>
          <button type="button" class="btn btn-sm" @click="read(m)">Minutes</button>
        </div>
        <p class="text-xs text-muted">{{ m.attendees.join(', ') }} · {{ minutesLong(m) }} min</p>
        <p v-if="m.comments[0]" class="mt-1 truncate text-xs italic">“{{ m.comments[0] }}”</p>
      </li>
    </ul>
  </div>

  <!-- On the page -->
  <template v-else>
    <div
      v-if="stub.running"
      class="sticky top-0 z-[6] -mx-4 mb-4 flex items-center gap-3 bg-accent px-4 py-2 text-on-accent shadow"
    >
      <span class="size-2 animate-pulse rounded-full bg-white" />
      <strong>Meeting</strong>
      <span class="font-mono">{{ elapsed }}</span>
      <span class="text-sm opacity-80"
        >since {{ fmtTime(stub.running.startedAt) }} ·
        {{ stub.running.comments.length }} Comments</span
      >
      <span class="flex-1" />
      <template v-if="stub.resumePending">
        <span class="text-sm">Restored from this browser.</span>
        <button type="button" class="btn btn-sm" @click="stub.resumePending = false">
          OK, carry on
        </button>
        <button type="button" class="btn btn-sm" @click="cancel">Cancel it</button>
      </template>
      <template v-else>
        <div class="relative">
          <button type="button" class="btn btn-sm" @click="composing = !composing">
            + Comment
          </button>
          <div
            v-if="composing"
            class="absolute right-0 top-full mt-2 w-96 rounded border border-border bg-surface p-3 text-text shadow-xl"
          >
            <form
              @submit.prevent="post"
            >
              <textarea
                v-model="draft"
                class="field w-full"
                rows="3"
                placeholder="Meeting Comment"
              />
              <div class="mt-2 flex justify-end gap-2">
                <button type="button" class="btn btn-sm" @click="composing = false">Close</button>
                <button type="submit" class="btn btn-sm btn-primary">Add</button>
              </div>
            </form>
            <ol class="mt-2 max-h-48 overflow-y-auto text-sm">
              <li
                v-for="c in [...stub.running.comments].reverse()"
                :key="c.id"
                class="flex gap-2 border-t border-divider py-1"
              >
                <span class="flex-1">{{ c.body }}</span>
                <button type="button" class="text-xs text-link" @click="removeComment(c.id)">
                  ×
                </button>
              </li>
            </ol>
          </div>
        </div>
        <button type="button" class="btn btn-sm" @click="openWrapUp">End…</button>
      </template>
    </div>

    <!-- Wrap-up dialog -->
    <div v-if="wrapUp" class="fixed inset-0 z-20 grid place-items-center bg-black/40">
      <div role="dialog" class="w-full max-w-lg rounded bg-surface p-5 shadow-xl">
        <h2 class="text-lg font-semibold">End the Meeting</h2>
        <p class="text-sm text-muted">
          {{ elapsed }} long, {{ stub.running?.comments.length }} Comments.
        </p>
        <fieldset class="mt-3">
          <legend class="font-medium">Who attended?</legend>
          <div class="mt-1 grid grid-cols-2 gap-1">
            <label v-for="m in members.data.value" :key="m.id" class="flex items-center gap-2">
              <input
                v-model="attendees"
                type="checkbox"
                :value="m.id"
                :disabled="m.id === me?.id"
              />{{ m.display_name }}
            </label>
          </div>
        </fieldset>
        <div class="mt-4 flex gap-2">
          <button type="button" class="btn btn-primary" @click="finish">Create Minutes</button>
          <button type="button" class="btn" @click="wrapUp = false">Back to Meeting</button>
          <span class="flex-1" />
          <button
            type="button"
            class="btn btn-danger"
            @click="cancel(); wrapUp = false"
          >
            Discard Meeting
          </button>
        </div>
      </div>
    </div>

    <!-- Minutes reader -->
    <div v-if="reader.meeting" class="fixed inset-0 z-20 overflow-y-auto bg-black/40 p-8">
      <div role="dialog" class="mx-auto max-w-3xl rounded bg-surface p-6 shadow-xl">
        <div class="mb-3 flex items-center">
          <h2 class="flex-1 text-lg font-semibold">
            Minutes · {{ fmtDate(reader.meeting.startedAt) }}
          </h2>
          <button type="button" class="btn" @click="reader.meeting = null">Close</button>
        </div>
        <MinutesView :meeting="reader.meeting" :project="project.project_code" wide />
      </div>
    </div>
  </template>
</template>
