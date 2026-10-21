<script setup lang="ts">
// PROTOTYPE (#75) Variant D, chosen so far: B's docked rail (leader always here,
// arrivals marked during the Meeting, attendees confirmed on ending) with C's
// wide Minutes reader, which opens over the page without closing the sidebar.
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

const reader = readerState
const confirming = ref(false)
const elapsed = useElapsed(() => stub.running?.startedAt)

function begin() {
  start(me.value ? [me.value.id] : []) // only the leader; tick others off as they join
  emit('close')
}
function toggle(id: string) {
  const r = stub.running!
  r.attendeeIds = r.attendeeIds.includes(id)
    ? r.attendeeIds.filter((a) => a !== id)
    : [...r.attendeeIds, id]
}
function askCancel() {
  if (window.confirm('Cancel the Meeting and discard its Comments?')) cancel()
}
function finish() {
  confirming.value = false
  end(name, me.value?.display_name ?? 'You')
  reader.meeting = stub.justEnded
  stub.justEnded = null
}
</script>

<script lang="ts">
import { reactive } from 'vue'
// Shared by the panel and page instances, so the reader sits over both.
const readerState = reactive({ meeting: null as PastMeeting | null })
</script>

<template>
  <!-- In the Project menu -->
  <div v-if="place === 'panel'" class="mt-4 grid gap-4">
    <template v-if="true">
      <p v-if="stub.running" class="rounded bg-accent-soft p-3 text-sm">
        Your Meeting is running in the rail on the right.
      </p>
      <p
        v-else-if="stub.role === 'other-leader-running'"
        class="rounded bg-st-amber-bg p-3 text-sm text-st-amber-fg"
      >
        Sam Okafor is running a Meeting. You can start one once it ends.
      </p>
      <button
        v-else-if="stub.role === 'leader'"
        type="button"
        class="btn btn-primary btn-lg"
        @click="begin"
      >
        Start a Meeting now
      </button>
      <p v-else class="text-sm text-muted">Only a Project Leader can start a Meeting.</p>
      <section>
        <h3 class="font-semibold">Past Meetings</h3>
        <ul class="mt-2 grid gap-1">
          <li v-for="m in stub.past" :key="m.id">
            <button
              type="button"
              class="w-full rounded px-2 py-1 text-left hover:bg-hover"
              @click="reader.meeting = m"
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

  <!-- Docked beside the page -->
  <template v-else-if="stub.running">
    <aside
      class="fixed inset-y-0 right-0 z-[5] flex flex-col border-l-4 border-accent bg-surface shadow-lg"
      :class="stub.railCollapsed ? 'w-12' : 'w-96'"
      aria-label="Meeting"
    >
      <button
        v-if="stub.railCollapsed"
        type="button"
        class="h-full font-mono text-sm [writing-mode:vertical-rl]"
        @click="stub.railCollapsed = false"
      >
        ● Meeting {{ elapsed }} · {{ stub.running?.comments.length ?? 0 }} Comments
      </button>
      <template v-else-if="stub.resumePending">
        <div class="m-4 grid gap-2">
          <p class="font-semibold">Pick up where you left off?</p>
          <p class="text-sm">
            Your Meeting from {{ fmtTime(stub.running!.startedAt) }} is still held by this browser.
          </p>
          <button type="button" class="btn btn-primary" @click="stub.resumePending = false">
            Carry on
          </button>
          <button type="button" class="btn" @click="cancel">Cancel the Meeting</button>
        </div>
      </template>
      <template v-else-if="stub.running">
        <header class="flex items-center gap-2 border-b border-divider p-3">
          <span class="size-2 animate-pulse rounded-full bg-st-red-solid" />
          <h2 class="flex-1 font-semibold">
            Meeting · <span class="font-mono">{{ elapsed }}</span>
          </h2>
          <button
            type="button"
            class="btn btn-sm btn-ghost"
            aria-label="Collapse"
            @click="stub.railCollapsed = true"
          >
            ⇥
          </button>
        </header>
        <div class="border-b border-divider p-3">
          <p class="mb-1 text-xs text-muted">Here (tap to mark arrivals)</p>
          <div class="flex flex-wrap gap-1">
            <span
              v-if="me"
              class="rounded-full border border-accent bg-accent px-2 py-0.5 text-xs text-on-accent"
              title="You're leading, so you're always here"
            >
              ✓ {{ me.display_name }} (leading)
            </span>
            <button
              v-for="m in (members.data.value ?? []).filter((x) => x.id !== me?.id)"
              :key="m.id"
              type="button"
              class="rounded-full border px-2 py-0.5 text-xs"
              :class="
                stub.running.attendeeIds.includes(m.id)
                  ? 'border-accent bg-accent text-on-accent'
                  : 'border-control-border'
              "
              :aria-pressed="stub.running.attendeeIds.includes(m.id)"
              @click="toggle(m.id)"
            >
              <template v-if="stub.running.attendeeIds.includes(m.id)">✓ </template>{{ m.display_name }}
            </button>
          </div>
        </div>
        <ol class="flex-1 overflow-y-auto p-3">
          <li
            v-for="c in stub.running.comments"
            :key="c.id"
            class="mb-2 rounded-lg bg-subtle p-2 text-sm"
          >
            {{ c.body }} <span class="ml-1 text-xs text-muted">{{ fmtTime(c.at) }}</span>
          </li>
          <li v-if="!stub.running.comments.length" class="text-sm text-muted">
            Edit items on the page as you go; their Changes go into the Minutes. Meeting-wide notes
            go here.
          </li>
        </ol>
        <form
          class="flex gap-2 border-t border-divider p-3"
          @submit.prevent="post"
        >
          <input v-model="draft" class="field flex-1" placeholder="Meeting Comment, Enter to add" />
        </form>
        <footer v-if="confirming" class="grid gap-2 border-t border-divider p-3">
          <p class="font-semibold">Who attended?</p>
          <p class="text-sm text-muted">
            {{ stub.running.attendeeIds.map(name).join(', ') }}. Mark anyone missing above.
          </p>
          <div class="flex gap-2">
            <button type="button" class="btn btn-primary flex-1" @click="finish">Create Minutes</button>
            <button type="button" class="btn" @click="confirming = false">Back</button>
          </div>
        </footer>
        <footer v-else class="flex gap-2 p-3 pt-0">
          <button type="button" class="btn btn-primary flex-1" @click="confirming = true">End Meeting…</button>
          <button type="button" class="btn" @click="askCancel">Cancel</button>
        </footer>
      </template>
    </aside>
  </template>
  <Teleport v-if="place === 'page'" to="body">
    <div v-if="reader.meeting" class="fixed inset-0 z-20 overflow-y-auto bg-black/40 p-8" @click.self="reader.meeting = null">
      <div role="dialog" class="mx-auto max-w-3xl rounded bg-surface p-6 shadow-xl">
        <div class="mb-3 flex items-center">
          <h2 class="flex-1 text-lg font-semibold">Minutes · {{ fmtDate(reader.meeting.startedAt) }}</h2>
          <button type="button" class="btn" @click="reader.meeting = null">Close</button>
        </div>
        <MinutesView :meeting="reader.meeting" :project="project.name" wide />
      </div>
    </div>
  </Teleport>
</template>
