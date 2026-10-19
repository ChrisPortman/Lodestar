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
import ItemPanel from './ItemPanel.vue'

const props = defineProps<{ project: Project }>()

const projectId = toRef(() => props.project.id)
const actions = useActions(projectId)
const members = useMembers(projectId)
const create = useCreateAction()
const update = useUpdateAction()
const close = useSetActionClosed()

const open = computed(() => (actions.data.value ?? []).filter((a) => !a.closed))
const closed = computed(() => (actions.data.value ?? []).filter((a) => a.closed))
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

    <p v-if="error" role="alert" class="mt-2 text-sm text-red-700">
      Couldn't save: {{ error.message }}
    </p>
    <p v-if="actions.isPending.value" class="mt-2 text-sm text-gray-600">Loading…</p>

    <template #item="{ item: action }">
      <div
        class="-mx-2 flex flex-wrap items-start gap-3 rounded px-2"
        :class="{ 'bg-amber-50': !action.closed && !action.assignee }"
        :data-unassigned="!action.closed && !action.assignee ? '' : undefined"
      >
        <span class="w-14 shrink-0 font-mono text-sm text-gray-700">{{ action.reference }}</span>
        <div class="min-w-0 flex-1">
          <DescriptionEditor
            v-if="editingId === action.id"
            :initial="action.description"
            :label="`Description of ${action.reference}`"
            @save="(text) => saveDescription(action, text).catch(() => {})"
            @cancel="editingId = null"
          />
          <p v-else class="whitespace-pre-line">{{ action.description }}</p>
        </div>
        <select
          :value="action.assignee?.id ?? ''"
          :aria-label="`Assignee of ${action.reference}`"
          :disabled="!writable"
          class="rounded border border-gray-300 text-sm"
          :class="{ 'border-amber-400 text-amber-900': !action.closed && !action.assignee }"
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
          class="rounded border border-gray-300 text-sm"
          :class="{ 'border-red-400 font-semibold text-red-700': isOverdue(action) }"
          @change="setDue(action, $event)"
        />
        <template v-if="writable">
          <button
            v-if="editingId !== action.id"
            type="button"
            :aria-label="`Edit ${action.reference}`"
            class="rounded border border-gray-300 px-2 py-0.5 text-sm hover:bg-gray-100"
            @click="editingId = action.id"
          >
            Edit
          </button>
          <button
            type="button"
            :aria-label="`${action.closed ? 'Reopen' : 'Close'} ${action.reference}`"
            class="rounded border border-gray-300 px-2 py-0.5 text-sm hover:bg-gray-100"
            @click="close.mutate({ action, closed: !action.closed })"
          >
            {{ action.closed ? 'Reopen' : 'Close' }}
          </button>
        </template>
      </div>
    </template>
  </ItemPanel>
</template>
