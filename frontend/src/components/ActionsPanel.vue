<script setup lang="ts">
// The Actions panel (FR-30 to FR-33, FR-23, FR-90 to FR-94): assignee and
// due date save straight away, descriptions need an explicit Save.
import { computed, ref, toRef } from 'vue'
import type { Project } from '../api/client'
import type { Action } from '../api/actions'
import {
  isOverdue,
  useActions,
  useCreateAction,
  useSetActionClosed,
  useUpdateAction,
} from '../actions'
import { useMembers } from '../members'
import DescriptionEditor from './DescriptionEditor.vue'
import CommentsSection from './CommentsSection.vue'
import ItemPanel from './ItemPanel.vue'
import ListControls from './ListControls.vue'
import { applyQuery, useListQuery } from '../listControls'

const props = defineProps<{ project: Project }>()

const projectId = toRef(() => props.project.id)
const actions = useActions(projectId)
const members = useMembers(projectId)
const create = useCreateAction()
const update = useUpdateAction()
const close = useSetActionClosed()

const query = useListQuery()
const SORTS = [
  { value: 'reference', label: 'Reference' },
  { value: 'due', label: 'Due date' },
  { value: 'assignee', label: 'Assignee' },
]
const shown = computed(() =>
  applyQuery(actions.data.value ?? [], query, {
    reference: (a) => a.reference,
    assignee: (a) => a.assignee ?? null,
    due: (a) => a.due_date ?? null,
  }),
)
const open = computed(() => (query.state === 'closed' ? [] : shown.value.filter((a) => !a.closed)))
const closed = computed(() => (query.state === 'open' ? [] : shown.value.filter((a) => a.closed)))
const writable = computed(() => !props.project.archived)
const error = computed(() => update.error.value ?? close.error.value ?? create.error.value)

/** Members to choose from, plus a former Member still on a closed Action (FR-22). */
function assigneeOptions(action: Action) {
  const list = members.data.value ?? []
  const a = action.assignee
  return a && !list.some((m) => m.id === a.id) ? [...list, a] : list
}

async function add(description: string, done: () => void) {
  await create.mutateAsync({
    projectId: props.project.id,
    fields: { description, assignee: null, due_date: null },
  })
  done()
}

const editingId = ref<string | null>(null)

async function saveDescription(action: Action, description: string) {
  await update.mutateAsync({ action, fields: { description } })
  editingId.value = null
}

function assign(action: Action, event: Event) {
  const id = (event.target as HTMLSelectElement).value
  const person = assigneeOptions(action).find((m) => m.id === id) ?? null
  update.mutate({ action, fields: { assignee: person } })
}

function setDue(action: Action, event: Event) {
  const value = (event.target as HTMLInputElement).value
  update.mutate({ action, fields: { due_date: value || null } })
}
</script>

<template>
  <ItemPanel
    title="Actions"
    item-label="Action"
    :open="open"
    :finalised="closed"
    :can-add="writable"
    :expand-finalised="query.state === 'closed'"
  >
    <template #add="{ done }">
      <DescriptionEditor
        initial=""
        label="Description"
        save-label="Add"
        @save="(text) => add(text, done).catch(() => {})"
        @cancel="done"
      />
    </template>

    <ListControls
      title="Actions"
      :query="query"
      :sorts="SORTS"
      :people="members.data.value ?? []"
      show-due
    />
    <p v-if="error" role="alert" class="mt-2 text-sm text-danger-text">
      Couldn't save: {{ error.message }}
    </p>
    <p v-if="actions.isPending.value" class="mt-2 text-sm text-muted">Loading…</p>

    <template #item="{ item: action }">
      <div
        class="-mx-2 flex flex-wrap items-start gap-3 rounded px-2"
        :class="{ 'bg-st-amber-bg/40': !action.closed && !action.assignee }"
        :data-unassigned="!action.closed && !action.assignee ? '' : undefined"
      >
        <span class="w-14 shrink-0 font-mono text-sm text-muted">{{ action.reference }}</span>
        <div class="min-w-0 flex-1">
          <DescriptionEditor
            v-if="editingId === action.id"
            :initial="action.description"
            :label="`Description of ${action.reference}`"
            @save="(text) => saveDescription(action, text).catch(() => {})"
            @cancel="editingId = null"
          />
          <p v-else class="whitespace-pre-line">{{ action.description }}</p>
          <p
            v-if="action.blocked_by.length > 0"
            class="mt-1 text-sm font-semibold text-danger-text"
            data-blocked
          >
            Blocked by {{ action.blocked_by.map((b) => b.reference).join(', ') }}
          </p>
        </div>
        <select
          :value="action.assignee?.id ?? ''"
          :aria-label="`Assignee of ${action.reference}`"
          :disabled="!writable"
          class="field text-sm"
          :class="{ 'border-st-amber-solid text-st-amber-fg': !action.closed && !action.assignee }"
          @change="assign(action, $event)"
        >
          <option value="">Unassigned</option>
          <option v-for="m in assigneeOptions(action)" :key="m.id" :value="m.id">
            {{ m.display_name }}
          </option>
        </select>
        <input
          type="date"
          :value="action.due_date ?? ''"
          :aria-label="`Due date of ${action.reference}`"
          :disabled="!writable"
          :data-overdue="String(isOverdue(action))"
          class="field text-sm"
          :class="{ 'border-danger font-semibold text-danger-text': isOverdue(action) }"
          @change="setDue(action, $event)"
        />
        <template v-if="writable">
          <button
            v-if="editingId !== action.id"
            type="button"
            :aria-label="`Edit ${action.reference}`"
            class="btn"
            @click="editingId = action.id"
          >
            Edit
          </button>
          <button
            type="button"
            :aria-label="`${action.closed ? 'Reopen' : 'Close'} ${action.reference}`"
            class="btn"
            @click="close.mutate({ action, closed: !action.closed })"
          >
            {{ action.closed ? 'Reopen' : 'Close' }}
          </button>
        </template>
      </div>
      <CommentsSection target-type="action" :target-id="action.id" :label="action.reference" />
    </template>
  </ItemPanel>
</template>
