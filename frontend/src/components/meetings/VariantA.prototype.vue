<script setup lang="ts">
// PROTOTYPE (#75) Variant A: step-by-step, all inside the Project menu panel.
// Home (start + past list) → pick attendees → running → Minutes. Nothing on the page.
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

const props = defineProps<{ project: Project }>()
const members = useMembers(computed(() => props.project.id))
const session = useSession()
const me = computed(() => session.data.value?.person)
const name = (id: string) => members.data.value?.find((m) => m.id === id)?.display_name ?? id

type Step = 'home' | 'attendees' | 'minutes'
const step = ref<Step>(stub.justEnded ? 'minutes' : 'home')
const viewing = ref<PastMeeting | null>(stub.justEnded)
const chosen = ref<string[]>([])
const draft = ref('')
function post() {
  addComment(draft.value)
  draft.value = ''
}
const confirmingCancel = ref(false)
const elapsed = useElapsed(() => stub.running?.startedAt)

function pickAttendees() {
  chosen.value = (members.data.value ?? []).map((m) => m.id) // default: everyone ticked
  step.value = 'attendees'
}
function begin() {
  start(chosen.value)
  step.value = 'home'
}
function finish() {
  end(name, me.value?.display_name ?? 'You')
  viewing.value = stub.justEnded
  step.value = 'minutes'
}
function open(m: PastMeeting) {
  viewing.value = m
  step.value = 'minutes'
}
</script>

<template>
  <div class="mt-4 grid gap-4">
    <!-- Resume after a crash or reload -->
    <div
      v-if="stub.resumePending"
      role="alert"
      class="rounded border border-st-amber-solid bg-st-amber-bg p-3 text-st-amber-fg"
    >
      <p class="font-semibold">You have a Meeting in progress</p>
      <p class="text-sm">
        Started {{ fmtTime(stub.running!.startedAt) }}, {{ stub.running!.comments.length }} Comments
        saved on this browser.
      </p>
      <div class="mt-2 flex gap-2">
        <button type="button" class="btn btn-sm btn-primary" @click="stub.resumePending = false">
          Resume
        </button>
        <button type="button" class="btn btn-sm" @click="cancel">Discard and release</button>
      </div>
    </div>

    <!-- Running -->
    <template v-else-if="stub.running">
      <div class="flex items-center gap-2 rounded bg-accent-soft p-3">
        <span class="size-2 animate-pulse rounded-full bg-st-red-solid" />
        <span class="flex-1 font-semibold">Meeting running</span>
        <span class="font-mono">{{ elapsed }}</span>
      </div>
      <p class="text-sm text-muted">
        Since {{ fmtTime(stub.running.startedAt) }}. Others can't edit this Project until you end or
        cancel.
      </p>
      <details class="text-sm">
        <summary class="cursor-pointer">{{ stub.running.attendeeIds.length }} attendees</summary>
        <ul class="mt-1 pl-4">
          <li v-for="id in stub.running.attendeeIds" :key="id">{{ name(id) }}</li>
        </ul>
      </details>
      <section>
        <h3 class="font-semibold">Meeting Comments</h3>
        <ol class="mt-2 grid gap-2">
          <li
            v-for="c in stub.running.comments"
            :key="c.id"
            class="group rounded border border-border p-2 text-sm"
          >
            <span class="text-xs text-muted">{{ fmtTime(c.at) }}</span>
            <p>{{ c.body }}</p>
            <button
              type="button"
              class="text-xs text-link opacity-0 group-hover:opacity-100"
              @click="removeComment(c.id)"
            >
              Remove
            </button>
          </li>
          <li v-if="!stub.running.comments.length" class="text-sm text-muted">None yet.</li>
        </ol>
        <form
          class="mt-2 grid gap-2"
          @submit.prevent="post"
        >
          <textarea
            v-model="draft"
            class="field"
            rows="3"
            placeholder="Note something about the Meeting…"
          />
          <button type="submit" class="btn btn-sm justify-self-end" :disabled="!draft.trim()">
            Add Comment
          </button>
        </form>
      </section>
      <div class="flex gap-2 border-t border-divider pt-3">
        <button type="button" class="btn btn-primary flex-1" @click="finish">
          End and create Minutes
        </button>
        <button type="button" class="btn" @click="confirmingCancel = true">Cancel</button>
      </div>
      <div
        v-if="confirmingCancel"
        role="alertdialog"
        class="rounded border border-danger p-3 text-sm"
      >
        Cancel the Meeting? Its Comments are thrown away and no Minutes are made. Changes made to
        items stay.
        <div class="mt-2 flex gap-2">
          <button
            type="button"
            class="btn btn-sm btn-danger"
            @click="cancel(); confirmingCancel = false"
          >
            Cancel Meeting
          </button>
          <button type="button" class="btn btn-sm" @click="confirmingCancel = false">
            Keep going
          </button>
        </div>
      </div>
    </template>

    <!-- Choosing attendees -->
    <template v-else-if="step === 'attendees'">
      <h3 class="font-semibold">Who's here?</h3>
      <fieldset class="grid gap-1">
        <label v-for="m in members.data.value" :key="m.id" class="flex items-center gap-2">
          <input v-model="chosen" type="checkbox" :value="m.id" :disabled="m.id === me?.id" />
          {{ m.display_name }}
          <span v-if="m.id === me?.id" class="text-xs text-muted">(you, leading)</span>
        </label>
      </fieldset>
      <div class="flex gap-2">
        <button type="button" class="btn btn-primary flex-1" @click="begin">Start Meeting</button>
        <button type="button" class="btn" @click="step = 'home'">Back</button>
      </div>
    </template>

    <!-- Minutes -->
    <template v-else-if="step === 'minutes' && viewing">
      <button type="button" class="btn btn-sm justify-self-start" @click="step = 'home'">
        ← All Meetings
      </button>
      <p
        v-if="viewing === stub.justEnded"
        class="rounded bg-st-green-bg p-2 text-sm text-st-green-fg"
      >
        Meeting created. The Project is unlocked.
      </p>
      <MinutesView :meeting="viewing" :project="project.project_code" />
    </template>

    <!-- Home -->
    <template v-else>
      <div
        v-if="stub.role === 'other-leader-running'"
        class="rounded bg-st-amber-bg p-3 text-sm text-st-amber-fg"
      >
        Meeting in progress, led by Sam Okafor, since
        {{ fmtTime(new Date(Date.now() - 12 * 60000)) }}.
      </div>
      <button
        v-else-if="stub.role === 'leader'"
        type="button"
        class="btn btn-primary btn-lg"
        @click="pickAttendees"
      >
        Start a Meeting
      </button>
      <p v-else class="text-sm text-muted">Only a Project Leader can start a Meeting.</p>

      <section>
        <h3 class="font-semibold">Past Meetings</h3>
        <ul class="mt-2 grid gap-1">
          <li v-for="m in stub.past" :key="m.id">
            <button
              type="button"
              class="w-full rounded px-2 py-1 text-left hover:bg-hover"
              @click="open(m)"
            >
              <span class="block">{{ fmtDate(m.startedAt) }}, {{ fmtTime(m.startedAt) }}</span>
              <span class="block text-xs text-muted">
                {{ minutesLong(m) }} min · led by {{ m.leader }} ·
                {{ m.attendees.length }} attendees
              </span>
            </button>
          </li>
        </ul>
      </section>
    </template>
  </div>
</template>
