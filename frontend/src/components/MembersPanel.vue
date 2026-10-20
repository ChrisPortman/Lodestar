<script setup lang="ts">
import { computed, ref, toRef } from 'vue'
import type { Project } from '../api/client'
import { useActions } from '../actions'
import { useBlockers } from '../blockers'
import { useAddMember, useMembers, useRemoveMember, useSetProjectLeader } from '../members'
import { usePeople } from '../session'

const props = defineProps<{ project: Project }>()
const emit = defineEmits<{ close: [] }>()

const projectId = toRef(() => props.project.id)
const members = useMembers(projectId)
const actions = useActions(projectId)
const blockers = useBlockers(projectId)
const people = usePeople()
const add = useAddMember()
const remove = useRemoveMember()
const lead = useSetProjectLeader()

const readOnly = computed(() => props.project.archived)
const candidates = computed(() => {
  const ids = new Set(members.data.value?.map((m) => m.id))
  return (people.data.value ?? []).filter((p) => !ids.has(p.id))
})
const error = computed(() => add.error.value ?? remove.error.value ?? lead.error.value)
const chosen = ref('')

/** Each Member's open Actions and open Blockers they're accountable for. */
const openItems = computed(() => {
  const counts = new Map<string, { actions: number; blockers: number }>()
  const entry = (id: string) => {
    let c = counts.get(id)
    if (!c) counts.set(id, (c = { actions: 0, blockers: 0 }))
    return c
  }
  for (const a of actions.data.value ?? [])
    if (!a.closed && a.assignee) entry(a.assignee.id).actions++
  for (const b of blockers.data.value ?? [])
    if (!b.cleared && b.accountable) entry(b.accountable.id).blockers++
  return counts
})

function plural(n: number, word: string) {
  return `${n} ${word}${n === 1 ? '' : 's'}`
}

function openItemsLabel(id: string) {
  const c = openItems.value.get(id) ?? { actions: 0, blockers: 0 }
  return `${plural(c.actions, 'open Action')}, ${plural(c.blockers, 'open Blocker')}`
}

async function addChosen() {
  if (!chosen.value) return
  await add.mutateAsync({ project: props.project, personId: chosen.value })
  chosen.value = ''
}
</script>

<template>
  <aside
    role="dialog"
    aria-labelledby="members-title"
    class="fixed inset-y-0 right-0 z-10 w-full max-w-sm overflow-y-auto border-l border-control-border bg-surface p-4 shadow-lg"
    @keydown.esc="emit('close')"
  >
    <div class="flex items-center">
      <h2 id="members-title" class="flex-1 text-lg font-semibold">Members</h2>
      <button
        type="button"
        class="rounded border border-control-border px-3 py-1 text-sm hover:bg-subtle"
        @click="emit('close')"
      >
        Close
      </button>
    </div>

    <p v-if="members.isPending.value" class="mt-4 text-muted">Loading…</p>
    <ul v-else aria-label="Members" class="mt-4 divide-y divide-divider">
      <li
        v-for="member in members.data.value"
        :key="member.id"
        class="flex items-center gap-2 py-2"
      >
        <span class="flex-1">
          {{ member.display_name }}
          <span
            v-if="project.project_leader?.id === member.id"
            class="ml-1 rounded bg-accent-soft px-2 py-0.5 text-xs text-link"
          >
            Leader
          </span>
          <span class="block text-xs text-muted" data-open-items>
            {{ openItemsLabel(member.id) }}
          </span>
        </span>
        <template v-if="!readOnly">
          <button
            v-if="project.project_leader?.id !== member.id"
            type="button"
            class="rounded border border-control-border px-2 py-0.5 text-xs hover:bg-subtle"
            :aria-label="`Make ${member.display_name} leader`"
            @click="lead.mutate({ project, leader: member })"
          >
            Make leader
          </button>
          <button
            type="button"
            class="rounded border border-control-border px-2 py-0.5 text-xs hover:bg-subtle"
            :aria-label="`Remove ${member.display_name}`"
            @click="remove.mutate({ project, personId: member.id })"
          >
            Remove
          </button>
        </template>
      </li>
    </ul>

    <form v-if="!readOnly" class="mt-4 flex gap-2" @submit.prevent="addChosen().catch(() => {})">
      <label class="sr-only" for="add-member">Add member</label>
      <select
        id="add-member"
        v-model="chosen"
        class="flex-1 rounded border border-control-border px-2 py-1"
      >
        <option value="" disabled>Choose a person…</option>
        <option v-for="person in candidates" :key="person.id" :value="person.id">
          {{ person.display_name }}
        </option>
      </select>
      <button
        type="submit"
        class="rounded bg-accent px-3 py-1 text-on-accent hover:bg-accent-hover"
        :disabled="!chosen"
      >
        Add
      </button>
    </form>
    <p v-if="error" role="alert" class="mt-2 text-danger-text">
      Couldn't change Members: {{ error.message }}
    </p>
  </aside>
</template>
