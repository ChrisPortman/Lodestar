<script setup lang="ts">
// The Blockers panel (FR-50 to FR-54, FR-90 to FR-94): the accountable
// Member saves straight away, descriptions need an explicit Save, and links
// to the Actions a Blocker blocks can be changed in any state.
import { computed, ref, toRef } from 'vue'
import type { Project } from '../api/client'
import type { Blocker } from '../api/blockers'
import { useActions } from '../actions'
import {
  useBlockers,
  useCreateBlocker,
  useLinkAction,
  useSetBlockerCleared,
  useUnlinkAction,
  useUpdateBlocker,
} from '../blockers'
import { useMembers } from '../members'
import DescriptionEditor from './DescriptionEditor.vue'
import CommentsSection from './CommentsSection.vue'
import ItemPanel from './ItemPanel.vue'
import ListControls from './ListControls.vue'
import { applyQuery, useListQuery } from '../listControls'

const props = defineProps<{ project: Project }>()

const projectId = toRef(() => props.project.id)
const blockers = useBlockers(projectId)
const actions = useActions(projectId)
const members = useMembers(projectId)
const create = useCreateBlocker()
const update = useUpdateBlocker()
const clear = useSetBlockerCleared()
const link = useLinkAction()
const unlink = useUnlinkAction()

const query = useListQuery()
const SORTS = [
  { value: 'reference', label: 'Reference' },
  { value: 'assignee', label: 'Accountable' },
]
const shown = computed(() =>
  applyQuery(blockers.data.value ?? [], query, {
    reference: (b) => b.reference,
    assignee: (b) => b.accountable ?? null,
  }),
)
const open = computed(() => (query.state === 'closed' ? [] : shown.value.filter((b) => !b.cleared)))
const cleared = computed(() => (query.state === 'open' ? [] : shown.value.filter((b) => b.cleared)))
const writable = computed(() => !props.project.archived)
const error = computed(
  () =>
    update.error.value ??
    clear.error.value ??
    create.error.value ??
    link.error.value ??
    unlink.error.value,
)

/** Members to choose from, plus a former Member still on a cleared Blocker (FR-22). */
function accountableOptions(blocker: Blocker) {
  const list = members.data.value ?? []
  const a = blocker.accountable
  return a && !list.some((m) => m.id === a.id) ? [...list, a] : list
}

/** The Project's Actions this Blocker does not yet block. */
function linkable(blocker: Blocker) {
  const linked = new Set(blocker.blocked_actions.map((a) => a.id))
  return (actions.data.value ?? []).filter((a) => !linked.has(a.id))
}

async function add(description: string, done: () => void) {
  await create.mutateAsync({
    projectId: props.project.id,
    fields: { description, accountable: null },
  })
  done()
}

const editingId = ref<string | null>(null)

async function saveDescription(blocker: Blocker, description: string) {
  await update.mutateAsync({ blocker, fields: { description } })
  editingId.value = null
}

function assign(blocker: Blocker, event: Event) {
  const id = (event.target as HTMLSelectElement).value
  const person = accountableOptions(blocker).find((m) => m.id === id) ?? null
  update.mutate({ blocker, fields: { accountable: person } })
}

function linkChosen(blocker: Blocker, event: Event) {
  const select = event.target as HTMLSelectElement
  if (!select.value) return
  link.mutate({ blocker, actionId: select.value })
  select.value = ''
}
</script>

<template>
  <ItemPanel
    title="Blockers"
    item-label="Blocker"
    :open="open"
    :finalised="cleared"
    finalised-label="Cleared"
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
      title="Blockers"
      :query="query"
      :sorts="SORTS"
      :people="members.data.value ?? []"
      person-label="accountable"
      finalised-label="Cleared"
    />
    <p v-if="error" role="alert" class="mt-2 text-sm text-red-700">
      Couldn't save: {{ error.message }}
    </p>
    <p v-if="blockers.isPending.value" class="mt-2 text-sm text-gray-600">Loading…</p>

    <template #item="{ item: blocker }">
      <div
        class="-mx-2 flex flex-wrap items-start gap-3 rounded px-2"
        :class="{ 'bg-amber-50': !blocker.cleared && !blocker.accountable }"
        :data-unassigned="!blocker.cleared && !blocker.accountable ? '' : undefined"
      >
        <span class="w-14 shrink-0 font-mono text-sm text-gray-700">{{ blocker.reference }}</span>
        <div class="min-w-0 flex-1">
          <DescriptionEditor
            v-if="editingId === blocker.id"
            :initial="blocker.description"
            :label="`Description of ${blocker.reference}`"
            @save="(text) => saveDescription(blocker, text).catch(() => {})"
            @cancel="editingId = null"
          />
          <p v-else class="whitespace-pre-line">{{ blocker.description }}</p>
          <ul
            :aria-label="`Actions blocked by ${blocker.reference}`"
            class="mt-1 flex flex-wrap gap-1 text-sm"
          >
            <li
              v-for="a in blocker.blocked_actions"
              :key="a.id"
              class="flex items-center gap-1 rounded bg-gray-100 px-2 py-0.5 font-mono"
            >
              {{ a.reference }}
              <button
                v-if="writable"
                type="button"
                :aria-label="`Unlink ${a.reference} from ${blocker.reference}`"
                class="text-gray-600 hover:text-red-700"
                @click="unlink.mutate({ blocker, actionId: a.id })"
              >
                ×
              </button>
            </li>
          </ul>
        </div>
        <select
          :value="blocker.accountable?.id ?? ''"
          :aria-label="`Accountable for ${blocker.reference}`"
          :disabled="!writable"
          class="rounded border border-gray-300 text-sm"
          :class="{ 'border-amber-400 text-amber-900': !blocker.cleared && !blocker.accountable }"
          @change="assign(blocker, $event)"
        >
          <option value="">Unassigned</option>
          <option v-for="m in accountableOptions(blocker)" :key="m.id" :value="m.id">
            {{ m.display_name }}
          </option>
        </select>
        <template v-if="writable">
          <select
            value=""
            :aria-label="`Link an Action to ${blocker.reference}`"
            class="rounded border border-gray-300 text-sm"
            @change="linkChosen(blocker, $event)"
          >
            <option value="">Link Action…</option>
            <option v-for="a in linkable(blocker)" :key="a.id" :value="a.id">
              {{ a.reference }}
            </option>
          </select>
          <button
            v-if="editingId !== blocker.id"
            type="button"
            :aria-label="`Edit ${blocker.reference}`"
            class="rounded border border-gray-300 px-2 py-0.5 text-sm hover:bg-gray-100"
            @click="editingId = blocker.id"
          >
            Edit
          </button>
          <button
            type="button"
            :aria-label="`${blocker.cleared ? 'Reopen' : 'Clear'} ${blocker.reference}`"
            class="rounded border border-gray-300 px-2 py-0.5 text-sm hover:bg-gray-100"
            @click="clear.mutate({ blocker, cleared: !blocker.cleared })"
          >
            {{ blocker.cleared ? 'Reopen' : 'Clear' }}
          </button>
        </template>
      </div>
      <CommentsSection target-type="blocker" :target-id="blocker.id" :label="blocker.reference" />
    </template>
  </ItemPanel>
</template>
