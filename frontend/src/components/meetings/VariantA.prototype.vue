<script setup lang="ts">
// PROTOTYPE (#75) Variant A: step-by-step, all inside the Project menu panel.
// Home (start + past list) → running → confirm attendees → Minutes reader.
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

const viewing = ref<PastMeeting | null>(stub.justEnded)
const chosen = ref<string[]>([])
const draft = ref('')
function post() {
  addComment(draft.value)
  draft.value = ''
}
const confirmingCancel = ref(false)
const elapsed = useElapsed(() => stub.running?.startedAt)

const wrappingUp = ref(false)
function begin() {
  start([])
}
// Attendees are confirmed at the end, so late arrivals are counted.
function wrapUp() {
  chosen.value = (members.data.value ?? []).map((m) => m.id) // default: everyone ticked
  wrappingUp.value = true
}
function finish() {
  stub.running!.attendeeIds = chosen.value
  wrappingUp.value = false
  end(name, me.value?.display_name ?? 'You')
  viewing.value = stub.justEnded
}
function open(m: PastMeeting) {
  viewing.value = m
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
      <div v-if="wrappingUp" class="grid gap-3 rounded border border-accent p-3">
        <h3 class="font-semibold">Who attended?</h3>
        <fieldset class="grid gap-1">
          <label v-for="m in members.data.value" :key="m.id" class="flex items-center gap-2">
            <input v-model="chosen" type="checkbox" :value="m.id" :disabled="m.id === me?.id" />
            {{ m.display_name }}
            <span v-if="m.id === me?.id" class="text-xs text-muted">(you, leading)</span>
          </label>
        </fieldset>
        <div class="flex gap-2">
          <button type="button" class="btn btn-primary flex-1" @click="finish">Create Minutes</button>
          <button type="button" class="btn" @click="wrappingUp = false">Back to Meeting</button>
        </div>
      </div>
      <div v-else class="flex gap-2 border-t border-divider pt-3">
        <button type="button" class="btn btn-primary flex-1" @click="wrapUp">
          End Meeting…
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

    <!-- Home -->
    <template v-else>
      <p v-if="stub.justEnded" class="rounded bg-st-green-bg p-2 text-sm text-st-green-fg">
        Meeting created. The Project is unlocked.
        <button type="button" class="text-link underline" @click="viewing = stub.justEnded">View Minutes</button>
      </p>
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
        @click="begin"
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
  <Teleport to="body">
    <div v-if="viewing" class="fixed inset-0 z-20 overflow-y-auto bg-black/40 p-8" @click.self="viewing = null">
      <div role="dialog" class="mx-auto max-w-3xl rounded bg-surface p-6 shadow-xl">
        <div class="mb-3 flex items-center">
          <h2 class="flex-1 text-lg font-semibold">Minutes · {{ fmtDate(viewing.startedAt) }}</h2>
          <button type="button" class="btn" @click="viewing = null">Close</button>
        </div>
        <MinutesView :meeting="viewing" :project="project.name" wide />
      </div>
    </div>
  </Teleport>
</template>
