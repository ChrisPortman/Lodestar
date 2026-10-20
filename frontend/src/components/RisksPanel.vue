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
  low: 'bg-green-100 text-green-800',
  medium: 'bg-amber-100 text-amber-900',
  high: 'bg-red-100 text-red-800',
} as const

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
            <select v-model.number="newLikelihood" class="rounded border border-gray-300">
              <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
            </select>
          </label>
          <label class="flex items-center gap-1">
            Impact
            <select v-model.number="newImpact" class="rounded border border-gray-300">
              <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
            </select>
          </label>
        </div>
      </DescriptionEditor>
    </template>

    <ListControls title="Risks" :query="query" :sorts="SORTS" />
    <p v-if="error" role="alert" class="mt-2 text-sm text-red-700">
      Couldn't save: {{ error.message }}
    </p>
    <p v-if="risks.isPending.value" class="mt-2 text-sm text-gray-600">Loading…</p>

    <template #item="{ item: risk }">
      <div class="flex flex-wrap items-start gap-3">
        <span class="w-14 shrink-0 font-mono text-sm text-gray-700">{{ risk.reference }}</span>
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
        <label class="flex items-center gap-1 text-sm">
          <span aria-hidden="true">L</span>
          <select
            :value="risk.likelihood"
            :aria-label="`Likelihood of ${risk.reference}`"
            :disabled="!writable"
            class="rounded border border-gray-300"
            @change="rate(risk, 'likelihood', $event)"
          >
            <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
          </select>
        </label>
        <label class="flex items-center gap-1 text-sm">
          <span aria-hidden="true">I</span>
          <select
            :value="risk.impact"
            :aria-label="`Impact of ${risk.reference}`"
            :disabled="!writable"
            class="rounded border border-gray-300"
            @change="rate(risk, 'impact', $event)"
          >
            <option v-for="n in RATINGS" :key="n" :value="n">{{ n }}</option>
          </select>
        </label>
        <span
          aria-label="Score"
          :data-severity="severity(risk.score)"
          class="w-10 rounded px-2 py-0.5 text-center text-sm font-semibold"
          :class="SEVERITY_CLASS[severity(risk.score)]"
        >
          {{ risk.score }}
        </span>
        <template v-if="writable">
          <button
            v-if="editingId !== risk.id"
            type="button"
            :aria-label="`Edit ${risk.reference}`"
            class="rounded border border-gray-300 px-2 py-0.5 text-sm hover:bg-gray-100"
            @click="editingId = risk.id"
          >
            Edit
          </button>
          <button
            type="button"
            :aria-label="`${risk.closed ? 'Reopen' : 'Close'} ${risk.reference}`"
            class="rounded border border-gray-300 px-2 py-0.5 text-sm hover:bg-gray-100"
            @click="close.mutate({ risk, closed: !risk.closed })"
          >
            {{ risk.closed ? 'Reopen' : 'Close' }}
          </button>
        </template>
      </div>
      <CommentsSection target-type="risk" :target-id="risk.id" :label="risk.reference" />
    </template>
  </ItemPanel>
</template>
