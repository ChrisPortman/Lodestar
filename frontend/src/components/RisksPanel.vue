<script setup lang="ts">
// The Risks panel (FR-40 to FR-42, FR-90 to FR-94): likelihood and impact
// save straight away, descriptions need an explicit Save.
import { computed, ref, toRef } from 'vue'
import type { Project } from '../api/client'
import type { Risk } from '../api/risks'
import { severity, useCreateRisk, useRisks, useSetRiskClosed, useUpdateRisk } from '../risks'
import DescriptionEditor from './DescriptionEditor.vue'
import CommentsSection from './CommentsSection.vue'
import ItemPanel from './ItemPanel.vue'
import ListControls from './ListControls.vue'
import { applyQuery, useListQuery } from '../listControls'

const props = defineProps<{ project: Project }>()

const risks = useRisks(toRef(() => props.project.id))
const create = useCreateRisk()
const update = useUpdateRisk()
const close = useSetRiskClosed()

const query = useListQuery()
const SORTS = [
  { value: 'reference', label: 'Reference' },
  { value: 'score', label: 'Score' },
]
const shown = computed(() =>
  applyQuery(risks.data.value ?? [], query, {
    reference: (r) => r.reference,
    score: (r) => r.score,
  }),
)
const open = computed(() => (query.state === 'closed' ? [] : shown.value.filter((r) => !r.closed)))
const closed = computed(() => (query.state === 'open' ? [] : shown.value.filter((r) => r.closed)))
const writable = computed(() => !props.project.archived)
const error = computed(() => update.error.value ?? close.error.value ?? create.error.value)

const RATINGS = [1, 2, 3, 4, 5]
const SEVERITY_CLASS = {
  low: 'bg-st-green-bg text-st-green-fg',
  medium: 'bg-st-amber-bg text-st-amber-fg',
  high: 'bg-st-red-bg text-st-red-fg',
} as const

/** High-score Risks are red, medium ones amber (#50). */
function accent(risk: Risk) {
  const s = severity(risk.score)
  return s === 'high' ? ('red' as const) : s === 'medium' ? ('amber' as const) : undefined
}

const newLikelihood = ref(3)
const newImpact = ref(3)

async function add(description: string, done: () => void) {
  await create.mutateAsync({
    projectId: props.project.id,
    fields: { description, likelihood: newLikelihood.value, impact: newImpact.value },
  })
  newLikelihood.value = 3
  newImpact.value = 3
  done()
}

const editingId = ref<string | null>(null)

async function saveDescription(risk: Risk, description: string) {
  await update.mutateAsync({ risk, fields: { description } })
  editingId.value = null
}

function rate(risk: Risk, field: 'likelihood' | 'impact', event: Event) {
  const value = Number((event.target as HTMLSelectElement).value)
  update.mutate({ risk, fields: { [field]: value } })
}
</script>

<template>
  <ItemPanel
    title="Risks"
    item-label="Risk"
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
      >
        <div class="flex gap-4 text-sm">
          <label class="flex items-center gap-1">
            Likelihood
            <select v-model.number="newLikelihood" class="rounded border border-control-border">
              <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
            </select>
          </label>
          <label class="flex items-center gap-1">
            Impact
            <select v-model.number="newImpact" class="rounded border border-control-border">
              <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
            </select>
          </label>
        </div>
      </DescriptionEditor>
    </template>

    <ListControls title="Risks" :query="query" :sorts="SORTS" />
    <p v-if="error" role="alert" class="mt-2 text-sm text-danger-text">
      Couldn't save: {{ error.message }}
    </p>
    <p v-if="risks.isPending.value" class="mt-2 text-sm text-muted">Loading…</p>

    <template #item="{ item: risk }">
      <div class="flex items-baseline gap-3">
        <span class="shrink-0 font-mono text-sm text-muted">{{ risk.reference }}</span>
        <div class="min-w-0 flex-1">
          <DescriptionEditor
            v-if="editingId === risk.id"
            :initial="risk.description"
            :label="`Description of ${risk.reference}`"
            @save="(text) => saveDescription(risk, text).catch(() => {})"
            @cancel="editingId = null"
          />
          <p v-else class="whitespace-pre-line">{{ risk.description }}</p>
        </div>
        <span
          aria-label="Score"
          :data-severity="severity(risk.score)"
          class="min-w-10 shrink-0 rounded px-2 py-0.5 text-center text-sm font-semibold tabular-nums"
          :class="SEVERITY_CLASS[severity(risk.score)]"
        >
          {{ risk.score }}
        </span>
      </div>
      <div class="mt-2 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
        <label class="flex items-center gap-2">
          <span class="text-xs tracking-wide text-muted uppercase">Likelihood</span>
          <select
            :value="risk.likelihood"
            :aria-label="`Likelihood of ${risk.reference}`"
            :disabled="!writable"
            class="rounded border border-control-border"
            @change="rate(risk, 'likelihood', $event)"
          >
            <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
          </select>
        </label>
        <label class="flex items-center gap-2">
          <span class="text-xs tracking-wide text-muted uppercase">Impact</span>
          <select
            :value="risk.impact"
            :aria-label="`Impact of ${risk.reference}`"
            :disabled="!writable"
            class="rounded border border-control-border"
            @change="rate(risk, 'impact', $event)"
          >
            <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
          </select>
        </label>
        <div v-if="writable" class="ml-auto flex gap-2">
          <button
            v-if="editingId !== risk.id"
            type="button"
            :aria-label="`Edit ${risk.reference}`"
            class="rounded border border-control-border px-2 py-0.5 text-sm hover:bg-subtle"
            @click="editingId = risk.id"
          >
            Edit
          </button>
          <button
            type="button"
            :aria-label="`${risk.closed ? 'Reopen' : 'Close'} ${risk.reference}`"
            class="rounded border border-control-border px-2 py-0.5 text-sm hover:bg-subtle"
            @click="close.mutate({ risk, closed: !risk.closed })"
          >
            {{ risk.closed ? 'Reopen' : 'Close' }}
          </button>
        </div>
      </div>
      <CommentsSection target-type="risk" :target-id="risk.id" :label="risk.reference" />
    </template>
  </ItemPanel>
</template>
