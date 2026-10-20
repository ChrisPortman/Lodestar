<script setup lang="ts" generic="T extends { id: string }">
// A meeting view panel (FR-90, FR-94): open items first, an Add form on
// demand, and finalised items in a collapsed list at the bottom.
import { ref } from 'vue'
import { CARD, accentClass, type Accent } from '../cards'

const props = withDefaults(
  defineProps<{
    /** The panel heading, e.g. "Risks"; also its accessible name. */
    title: string
    /** One item, e.g. "Risk", for "Add Risk" and "Open Risks". */
    itemLabel: string
    open: T[]
    finalised: T[]
    /** Heading of the collapsed list, e.g. "Closed" or "Cleared". */
    finalisedLabel?: string
    canAdd?: boolean
    /** Show the finalised list expanded, e.g. when filtered to it. */
    expandFinalised?: boolean
    /** The card's state accent (#50): red or amber, or none. Never the only cue. */
    accent?: (item: T) => Accent | undefined
  }>(),
  { finalisedLabel: 'Closed', canAdd: true, expandFinalised: false, accent: () => undefined },
)

defineSlots<{
  item(props: { item: T }): unknown
  add(props: { done: () => void }): unknown
  default(): unknown
}>()

const adding = ref(false)
const headingId = `panel-${props.title.toLowerCase().replace(/\W+/g, '-')}`
</script>

<template>
  <section :aria-labelledby="headingId" class="rounded border border-border bg-surface p-4">
    <div class="flex items-center gap-2">
      <h2 :id="headingId" class="flex-1 text-lg font-semibold">{{ title }}</h2>
      <button
        v-if="canAdd && !adding"
        type="button"
        :aria-label="`Add ${itemLabel}`"
        class="btn"
        @click="adding = true"
      >
        + Add
      </button>
    </div>
    <div v-if="adding" class="mt-3">
      <slot name="add" :done="() => (adding = false)" />
    </div>
    <slot />
    <ul :aria-label="`Open ${title}`" class="mt-3 grid gap-2">
      <li
        v-for="item in open"
        :key="item.id"
        :data-accent="accent(item)"
        :class="[CARD, accentClass(accent(item))]"
      >
        <slot name="item" :item="item" />
      </li>
    </ul>
    <p v-if="open.length === 0" class="text-sm text-muted">No open {{ title }}.</p>
    <details v-if="finalised.length > 0" class="mt-3" :open="expandFinalised">
      <summary class="cursor-pointer text-sm text-muted">
        {{ finalisedLabel }} ({{ finalised.length }})
      </summary>
      <ul :aria-label="`${finalisedLabel} ${title}`" class="mt-2 grid gap-2 opacity-75">
        <li v-for="item in finalised" :key="item.id" :class="[CARD, 'border-l-border']">
          <slot name="item" :item="item" />
        </li>
      </ul>
    </details>
  </section>
</template>
