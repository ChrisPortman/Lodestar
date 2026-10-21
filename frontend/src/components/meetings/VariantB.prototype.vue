<script setup lang="ts">
// PROTOTYPE (#75) Variant B: the menu panel only starts Meetings and lists history.
// Once running, the Meeting moves into a docked, non-modal rail beside the page
// (place="page"), so the leader edits items with the rail open. Attendance is
// taken in the rail during the Meeting; Comments are a chat-style feed.
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
const collapsed = ref(false)
const viewing = ref<PastMeeting | null>(null)
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
  end(name, me.value?.display_name ?? 'You')
}
</script>

<template>
  <!-- In the Project menu -->
  <div v-if="place === 'panel'" class="mt-4 grid gap-4">
    <template v-if="viewing">
      <button type="button" class="btn btn-sm justify-self-start" @click="viewing = null">
        ← History
      </button>
      <MinutesView :meeting="viewing" :project="project.project_code" />
    </template>
    <template v-else>
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
      <table class="w-full text-sm">
        <caption class="text-left font-semibold">
          History
        </caption>
        <thead class="text-xs text-muted">
          <tr>
            <th class="text-left">When</th>
            <th class="text-left">Leader</th>
            <th class="text-right">Mins</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="m in stub.past"
            :key="m.id"
            class="cursor-pointer hover:bg-hover"
            @click="viewing = m"
          >
            <td class="py-1">{{ fmtDate(m.startedAt) }}</td>
            <td>{{ m.leader }}</td>
            <td class="text-right font-mono">{{ minutesLong(m) }}</td>
          </tr>
        </tbody>
      </table>
    </template>
  </div>

  <!-- Docked beside the page -->
  <template v-else-if="stub.running || stub.justEnded">
    <aside
      class="fixed inset-y-0 right-0 z-[5] flex flex-col border-l-4 border-accent bg-surface shadow-lg"
      :class="collapsed ? 'w-12' : 'w-96'"
      aria-label="Meeting"
    >
      <button
        v-if="collapsed"
        type="button"
        class="h-full font-mono text-sm [writing-mode:vertical-rl]"
        @click="collapsed = false"
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
            @click="collapsed = true"
          >
            ⇥
          </button>
        </header>
        <div class="border-b border-divider p-3">
          <p class="mb-1 text-xs text-muted">Here (tap to mark)</p>
          <div class="flex flex-wrap gap-1">
            <button
              v-for="m in members.data.value"
              :key="m.id"
              type="button"
              class="rounded-full border px-2 py-0.5 text-xs"
              :class="
                stub.running.attendeeIds.includes(m.id)
                  ? 'border-accent bg-accent text-on-accent'
                  : 'border-control-border'
              "
              :disabled="m.id === me?.id"
              @click="toggle(m.id)"
            >
              {{ m.display_name }}
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
        <footer class="flex gap-2 p-3 pt-0">
          <button type="button" class="btn btn-primary flex-1" @click="finish">End Meeting</button>
          <button type="button" class="btn" @click="askCancel">Cancel</button>
        </footer>
      </template>
      <template v-else-if="stub.justEnded">
        <header class="flex items-center gap-2 border-b border-divider p-3">
          <h2 class="flex-1 font-semibold">Minutes</h2>
          <button type="button" class="btn btn-sm" @click="stub.justEnded = null">Done</button>
        </header>
        <div class="overflow-y-auto p-3">
          <MinutesView :meeting="stub.justEnded" :project="project.project_code" />
        </div>
      </template>
    </aside>
  </template>
</template>
