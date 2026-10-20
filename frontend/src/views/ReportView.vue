<script setup lang="ts">
import { computed, toRef } from 'vue'
import { isOverdue, today, useActions } from '../actions'
import { useBlockers } from '../blockers'
import { useProject } from '../projects'
import { severity, useRisks } from '../risks'

// Project on a Page (FR-100, FR-101): a printable snapshot built from the API.
const props = defineProps<{ id: string }>()
const id = toRef(props, 'id')
const project = useProject(id)
const actions = useActions(id)
const blockers = useBlockers(id)
const risks = useRisks(id)
const generated = today()

const openActions = computed(() => (actions.data.value ?? []).filter((a) => !a.closed))
const openBlockers = computed(() => (blockers.data.value ?? []).filter((b) => !b.cleared))
const openRisks = computed(() =>
  (risks.data.value ?? []).filter((r) => !r.closed).sort((a, b) => b.score - a.score),
)

const RATING = {
  red: { label: 'Red', badge: 'bg-st-red-bg text-st-red-fg' },
  amber: { label: 'Amber', badge: 'bg-st-amber-bg text-st-amber-fg' },
  green: { label: 'Green', badge: 'bg-st-green-bg text-st-green-fg' },
  none: { label: 'Not rated', badge: 'bg-active text-muted' },
} as const
const rating = computed(() => RATING[project.data.value?.health_rating?.rating ?? 'none'])

const SEVERITY_CLASS = {
  low: 'bg-st-green-bg text-st-green-fg',
  medium: 'bg-st-amber-bg text-st-amber-fg',
  high: 'bg-st-red-bg text-st-red-fg',
} as const

const refs = (items: { reference: string }[]) => items.map((i) => i.reference).join(', ')
</script>

<template>
  <article class="report text-sm">
    <p v-if="project.isPending.value" class="text-muted">Loading…</p>
    <p v-else-if="project.error.value" role="alert" class="text-danger-text">
      Couldn't load the Project: {{ project.error.value.message }}
    </p>
    <template v-else-if="project.data.value">
      <header class="flex flex-wrap items-start gap-4 border-b-2 border-text pb-3">
        <div class="flex-1">
          <p class="font-mono text-xs text-muted">
            {{ project.data.value.project_code }} · Project on a Page
          </p>
          <h1 class="text-2xl font-semibold">{{ project.data.value.name }}</h1>
          <p>Project Leader: {{ project.data.value.project_leader?.display_name ?? 'none' }}</p>
        </div>
        <div class="text-right">
          <span class="rounded px-3 py-1 font-semibold" :class="rating.badge">{{
            rating.label
          }}</span>
          <p class="mt-1 text-xs text-muted">Generated {{ generated }}</p>
        </div>
      </header>
      <blockquote
        v-if="project.data.value.health_rating?.comment"
        class="mt-3 border-l-4 border-control-border pl-3 italic"
      >
        “{{ project.data.value.health_rating.comment }}”
      </blockquote>
      <p v-if="project.data.value.description" class="mt-3 whitespace-pre-line">
        {{ project.data.value.description }}
      </p>

      <table class="mt-6 w-full border-collapse" aria-labelledby="report-actions">
        <caption id="report-actions" class="text-left text-base font-semibold">
          Open Actions
        </caption>
        <thead>
          <tr class="border-b border-control-border text-left">
            <th class="pr-2">Ref</th>
            <th class="pr-2">Description</th>
            <th class="pr-2">Assignee</th>
            <th class="pr-2">Due</th>
            <th>Blocked by</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="a in openActions" :key="a.id" class="break-inside-avoid border-b align-top">
            <td class="pr-2 font-mono">{{ a.reference }}</td>
            <td class="pr-2 whitespace-pre-line">{{ a.description }}</td>
            <td class="pr-2">
              <span v-if="a.assignee">{{ a.assignee.display_name }}</span>
              <span
                v-else
                data-unassigned="true"
                class="rounded bg-st-amber-bg px-1 text-st-amber-fg"
                >Unassigned</span
              >
            </td>
            <td class="pr-2">
              <span
                v-if="a.due_date"
                :data-overdue="String(isOverdue(a, generated))"
                :class="{ 'font-semibold text-danger-text': isOverdue(a, generated) }"
                >{{ a.due_date }}</span
              >
            </td>
            <td class="font-mono">{{ refs(a.blocked_by) }}</td>
          </tr>
        </tbody>
      </table>

      <table class="mt-6 w-full border-collapse" aria-labelledby="report-blockers">
        <caption id="report-blockers" class="text-left text-base font-semibold">
          Open Blockers
        </caption>
        <thead>
          <tr class="border-b border-control-border text-left">
            <th class="pr-2">Ref</th>
            <th class="pr-2">Description</th>
            <th class="pr-2">Accountable</th>
            <th>Blocks</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="b in openBlockers" :key="b.id" class="break-inside-avoid border-b align-top">
            <td class="pr-2 font-mono">{{ b.reference }}</td>
            <td class="pr-2 whitespace-pre-line">{{ b.description }}</td>
            <td class="pr-2">{{ b.accountable?.display_name ?? '' }}</td>
            <td class="font-mono">{{ refs(b.blocked_actions) }}</td>
          </tr>
        </tbody>
      </table>

      <table class="mt-6 w-full border-collapse" aria-labelledby="report-risks">
        <caption id="report-risks" class="text-left text-base font-semibold">
          Open Risks
        </caption>
        <thead>
          <tr class="border-b border-control-border text-left">
            <th class="pr-2">Ref</th>
            <th class="pr-2">Description</th>
            <th class="pr-2">L</th>
            <th class="pr-2">I</th>
            <th>Score</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in openRisks" :key="r.id" class="break-inside-avoid border-b align-top">
            <td class="pr-2 font-mono">{{ r.reference }}</td>
            <td class="pr-2 whitespace-pre-line">{{ r.description }}</td>
            <td class="pr-2">{{ r.likelihood }}</td>
            <td class="pr-2">{{ r.impact }}</td>
            <td>
              <span
                class="rounded px-2 font-semibold"
                :class="SEVERITY_CLASS[severity(r.score)]"
                :data-severity="severity(r.score)"
                >{{ r.score }}</span
              >
            </td>
          </tr>
        </tbody>
      </table>
    </template>
  </article>
</template>
