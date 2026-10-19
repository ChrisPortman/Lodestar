<script setup lang="ts">
// Sort and filter controls for a meeting view panel (FR-92, FR-23).
import type { ListQuery } from '../listControls'

const props = withDefaults(
  defineProps<{
    /** The panel title, e.g. "Actions", for the controls' accessible names. */
    title: string
    query: ListQuery
    sorts: { value: string; label: string }[]
    /** People to filter by; no assignee filter when omitted. */
    people?: { id: string; display_name: string }[]
    /** What an item's Person is called, e.g. "assignee" or "accountable". */
    personLabel?: string
    /** Heading of finalised items, e.g. "Closed" or "Cleared". */
    finalisedLabel?: string
    showDue?: boolean
  }>(),
  { people: undefined, personLabel: 'assignee', finalisedLabel: 'Closed', showDue: false },
)

const unassignedOnly = () => props.query.assignee === 'unassigned'

function toggleUnassigned() {
  // eslint-disable-next-line vue/no-mutating-props -- the query is a shared reactive object
  props.query.assignee = unassignedOnly() ? '' : 'unassigned'
}
</script>

<template>
  <div class="mt-2 flex flex-wrap items-center gap-2 text-sm">
    <!-- eslint-disable vue/no-mutating-props -- the query is a shared reactive object -->
    <select
      v-model="query.sort"
      :aria-label="`Sort ${title}`"
      class="rounded border border-gray-300 text-sm"
    >
      <option v-for="s in sorts" :key="s.value" :value="s.value">Sort: {{ s.label }}</option>
    </select>
    <select
      v-model="query.state"
      :aria-label="`Filter ${title} by state`"
      class="rounded border border-gray-300 text-sm"
    >
      <option value="all">Open and {{ finalisedLabel.toLowerCase() }}</option>
      <option value="open">Open only</option>
      <option value="closed">{{ finalisedLabel }} only</option>
    </select>
    <template v-if="people">
      <select
        v-model="query.assignee"
        :aria-label="`Filter ${title} by ${personLabel}`"
        class="rounded border border-gray-300 text-sm"
      >
        <option value="">Any {{ personLabel }}</option>
        <option value="unassigned">Unassigned</option>
        <option v-for="p in people" :key="p.id" :value="p.id">{{ p.display_name }}</option>
      </select>
      <button
        type="button"
        :aria-pressed="unassignedOnly()"
        class="rounded border px-2 py-0.5"
        :class="
          unassignedOnly()
            ? 'border-amber-400 bg-amber-50 text-amber-900'
            : 'border-gray-300 hover:bg-gray-100'
        "
        @click="toggleUnassigned"
      >
        Unassigned only
      </button>
    </template>
    <label v-if="showDue" class="flex items-center gap-1">
      Due by
      <input v-model="query.dueBy" type="date" class="rounded border border-gray-300 text-sm" />
    </label>
  </div>
</template>
