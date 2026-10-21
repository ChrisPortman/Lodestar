<script setup lang="ts">
// The Meeting panel (#85): the leader's running Meeting, over the page at the
// Project menu's place, with a blue left edge. Close and Esc only hide it.
import { computed, ref } from 'vue'
import { clockTime } from '../meetingLock'
import { useMembers } from '../members'
import { useRunningMeeting } from '../runningMeeting'

const props = defineProps<{ projectId: string }>()
const meeting = useRunningMeeting()
const members = useMembers(computed(() => props.projectId))

const step = ref<'run' | 'ending' | 'cancelling'>('run')
const draft = ref('')
const busy = ref(false)
const error = ref('')

const close = () => (meeting.panelOpen.value = false)
const here = computed(() => (members.data.value ?? []).filter((m) => meeting.isHere(m.id)))

function addComment() {
  const body = draft.value.trim()
  if (!body) return
  meeting.addComment(body)
  draft.value = ''
}

async function run(what: () => Promise<void>) {
  busy.value = true
  error.value = ''
  try {
    await what()
    step.value = 'run'
  } catch (e) {
    if (!meeting.lost.value) error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <aside
    role="dialog"
    aria-labelledby="meeting-panel-title"
    class="fixed inset-y-0 right-0 z-10 w-full max-w-sm overflow-y-auto border-l-4 border-accent bg-surface p-4 shadow-lg"
    @keydown.esc="close"
  >
    <div class="flex items-center gap-2">
      <h2 id="meeting-panel-title" class="text-lg font-semibold">Meeting</h2>
      <span role="timer" aria-label="Elapsed" class="flex-1 font-mono text-muted">
        {{ meeting.elapsed.value }}
      </span>
      <button type="button" class="btn" @click="close">Close</button>
    </div>

    <template v-if="meeting.lost.value">
      <p role="alert" class="mt-4 rounded bg-st-amber-bg p-3 text-sm text-st-amber-fg">
        Someone else has taken over this Project's Meeting, so these Minutes can't be created. Your
        Comments are below so you can copy them.
      </p>
      <button type="button" class="btn mt-2" @click="meeting.dismiss()">Put away</button>
    </template>

    <div v-else-if="meeting.needsResume.value" class="mt-4 grid gap-2">
      <p>Your Meeting is still running.</p>
      <button type="button" class="btn btn-primary" @click="meeting.carryOn()">Carry on</button>
      <button type="button" class="btn" @click="step = 'cancelling'">Cancel the Meeting</button>
    </div>

    <div v-if="step === 'cancelling' && !meeting.lost.value" class="mt-4 grid gap-2">
      <p>Cancel the Meeting? Its Comments will be discarded and no Minutes created.</p>
      <button type="button" class="btn btn-danger" :disabled="busy" @click="run(meeting.cancel)">
        Yes, cancel it
      </button>
      <button type="button" class="btn" @click="step = 'run'">Keep going</button>
    </div>
    <template v-else-if="!meeting.needsResume.value && !meeting.lost.value">
      <section aria-labelledby="meeting-here-title" class="mt-4">
        <h3 id="meeting-here-title" class="font-semibold">
          {{ step === 'ending' ? 'Who attended?' : 'Here' }}
        </h3>
        <p v-if="step === 'ending'" class="text-sm text-muted">
          {{ here.map((m) => m.display_name).join(', ') }}. Mark anyone missing.
        </p>
        <ul class="mt-2 grid gap-1">
          <li v-for="m in members.data.value" :key="m.id">
            <label class="flex items-center gap-2">
              <input
                type="checkbox"
                :checked="meeting.isHere(m.id)"
                :disabled="m.id === meeting.leaderId.value"
                @change="meeting.toggleHere(m.id)"
              />
              {{ m.display_name }}
              <span v-if="m.id === meeting.leaderId.value" class="text-xs text-muted">leader</span>
            </label>
          </li>
        </ul>
      </section>
      <div v-if="step === 'ending'" class="mt-4 flex gap-2">
        <button type="button" class="btn btn-primary" :disabled="busy" @click="run(meeting.end)">
          Create Minutes
        </button>
        <button type="button" class="btn" @click="step = 'run'">Back</button>
      </div>
    </template>

    <section
      v-if="!meeting.needsResume.value && step !== 'cancelling'"
      aria-labelledby="meeting-comments-title"
      class="mt-4"
    >
      <h3 id="meeting-comments-title" class="font-semibold">Meeting Comments</h3>
      <ol aria-label="Meeting Comments" class="mt-2 grid gap-2">
        <li
          v-for="(c, i) in meeting.state.comments"
          :key="i"
          class="rounded bg-subtle px-3 py-2 text-sm"
        >
          <span class="block whitespace-pre-wrap">{{ c.body }}</span>
          <span class="block text-xs text-muted">{{ clockTime(c.at) }}</span>
        </li>
      </ol>
      <template v-if="!meeting.lost.value && step === 'run'">
        <label for="meeting-comment" class="sr-only">Meeting Comment</label>
        <textarea
          id="meeting-comment"
          v-model="draft"
          rows="2"
          placeholder="Add a Comment, then Enter"
          class="mt-2 w-full rounded border border-control-border bg-surface p-2 text-sm"
          @keydown.enter.exact.prevent="addComment"
        />
      </template>
    </section>

    <div
      v-if="step === 'run' && !meeting.needsResume.value && !meeting.lost.value"
      class="mt-4 flex gap-2"
    >
      <button type="button" class="btn btn-primary" @click="step = 'ending'">End Meeting…</button>
      <button type="button" class="btn" @click="step = 'cancelling'">Cancel</button>
    </div>
    <p v-if="error" role="alert" class="mt-2 text-danger-text">{{ error }}</p>
  </aside>
</template>
