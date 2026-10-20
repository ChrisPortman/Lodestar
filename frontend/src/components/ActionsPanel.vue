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
import { useEnsureMember, useMembers } from '../members'
import type { PersonRef } from '../api/members'
import NewPersonField from './NewPersonField.vue'
import DescriptionEditor from './DescriptionEditor.vue'
import CommentsSection from './CommentsSection.vue'
import ItemPanel from './ItemPanel.vue'
import ListControls from './ListControls.vue'
import { applyQuery, useListQuery } from '../listControls'
import { NEW_PERSON } from '../session'

const props = defineProps<{ project: Project }>()

const projectId = toRef(() => props.project.id)
const actions = useActions(projectId)
const members = useMembers(projectId)
const create = useCreateAction()
const update = useUpdateAction()
const close = useSetActionClosed()
const ensureMember = useEnsureMember()

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
const error = computed(
  () => update.error.value ?? close.error.value ?? create.error.value ?? ensureMember.error.value,
)

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

/** Overdue or blocked Actions are red, unassigned ones amber (#50). */
function accent(action: Action) {
  if (action.closed) return undefined
  if (isOverdue(action) || action.blocked_by.length > 0) return 'red' as const
  if (!action.assignee) return 'amber' as const
  return undefined
}

const editingId = ref<string | null>(null)

async function saveDescription(action: Action, description: string) {
  await update.mutateAsync({ action, fields: { description } })
  editingId.value = null
}

/** The Action whose assignee is being picked as a new Person (#59). */
const newPersonFor = ref<string | null>(null)

function assign(action: Action, event: Event) {
  const id = (event.target as HTMLSelectElement).value
  if (id === NEW_PERSON) {
    newPersonFor.value = action.id
    return
  }
  const person = assigneeOptions(action).find((m) => m.id === id) ?? null
  update.mutate({ action, fields: { assignee: person } })
}

async function assignNew(action: Action, person: PersonRef) {
  await ensureMember.ensure(props.project, person, members.data.value ?? [])
  await update.mutateAsync({ action, fields: { assignee: person } })
  newPersonFor.value = null
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
    :accent="accent"
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
      <div :data-unassigned="!action.closed && !action.assignee ? '' : undefined">
        <div class="flex items-baseline gap-3">
          <span class="shrink-0 font-mono text-sm text-muted">{{ action.reference }}</span>
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
        </div>
        <div class="mt-2 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
          <p
            v-if="action.blocked_by.length > 0"
            class="font-semibold text-danger-text"
            data-blocked
          >
            Blocked by {{ action.blocked_by.map((b) => b.reference).join(', ') }}
          </p>
          <label class="flex items-center gap-2">
            <span class="text-xs tracking-wide text-muted uppercase">Assignee</span>
            <NewPersonField
              v-if="newPersonFor === action.id"
              :label="`New assignee of ${action.reference}`"
              @picked="(person) => assignNew(action, person).catch(() => {})"
              @cancel="newPersonFor = null"
            />
            <select
              v-else
              :value="action.assignee?.id ?? ''"
              :aria-label="`Assignee of ${action.reference}`"
              :disabled="!writable"
              class="field text-sm"
              :class="{
                'border-st-amber-solid text-st-amber-fg': !action.closed && !action.assignee,
              }"
              @change="assign(action, $event)"
            >
              <option value="">Unassigned</option>
              <option v-for="m in assigneeOptions(action)" :key="m.id" :value="m.id">
                {{ m.display_name }}
              </option>
              <option :value="NEW_PERSON">New Person…</option>
            </select>
          </label>
          <label class="flex items-center gap-2">
            <span class="text-xs tracking-wide text-muted uppercase">Due</span>
            <input
              type="date"
              :value="action.due_date ?? ''"
              :aria-label="`Due date of ${action.reference}`"
              :disabled="!writable"
              :data-overdue="String(isOverdue(action))"
              class="field text-sm tabular-nums"
              :class="{ 'border-danger font-semibold text-danger-text': isOverdue(action) }"
              @change="setDue(action, $event)"
            />
          </label>
          <div v-if="writable" class="ml-auto flex gap-2">
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
          </div>
        </div>
      </div>
      <CommentsSection target-type="action" :target-id="action.id" :label="action.reference" />
    </template>
  </ItemPanel>
</template>
