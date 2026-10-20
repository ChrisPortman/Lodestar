<script setup lang="ts">
import { computed, toRef } from 'vue'
import { isOverdue, today, useActions } from '../actions'
import { useBlockers } from '../blockers'
import { useProject } from '../projects'
import { severity, useRisks } from '../risks'
import { RATING, SEVERITY, TONE_TEXT, tag } from '../status'

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

// Colour is never the only cue (#51): each status pairs a symbol and a word (status.ts), and
// the Health Rating outline is solid, dashed or dotted so it survives greyscale.
const OUTLINE = { red: 'solid', amber: 'dashed', green: 'dotted', none: 'solid' } as const
const ratingKey = computed(() => project.data.value?.health_rating?.rating ?? 'none')
const rating = computed(() => RATING[ratingKey.value])
const outline = computed(() => OUTLINE[ratingKey.value])

const refs = (items: { reference: string }[]) => items.map((i) => i.reference).join(', ')
</script>

<template>
  <article class="report">
    <p v-if="project.isPending.value" class="muted">Loading…</p>
    <p v-else-if="project.error.value" role="alert" class="text-danger-text">
      Couldn't load the Project: {{ project.error.value.message }}
    </p>
    <template v-else-if="project.data.value">
      <header class="masthead">
        <div class="flex-1">
          <p class="label">{{ project.data.value.project_code }} · Project on a Page</p>
          <h1>{{ project.data.value.name }}</h1>
          <p class="cell">
            Project Leader: {{ project.data.value.project_leader?.display_name ?? 'none' }}
          </p>
        </div>
        <div class="text-right">
          <span
            class="pill"
            :class="TONE_TEXT[rating.tone]"
            :data-outline="outline"
            :style="{ borderStyle: outline }"
            >{{ tag(rating) }}</span
          >
          <p class="meta">Generated {{ generated }}</p>
        </div>
      </header>
      <blockquote v-if="project.data.value.health_rating?.comment" class="comment">
        “{{ project.data.value.health_rating.comment }}”
      </blockquote>
      <p v-if="project.data.value.description" class="prose whitespace-pre-line">
        {{ project.data.value.description }}
      </p>

      <section>
        <h2>
          <span id="report-actions">Open Actions</span>
          <span class="count">{{ openActions.length }}</span>
        </h2>
        <table aria-labelledby="report-actions">
          <colgroup>
            <col class="w-ref" />
            <col />
            <col class="w-people" />
            <col class="w-due" />
            <col class="w-refs" />
          </colgroup>
          <thead>
            <tr>
              <th>Ref</th>
              <th>Description</th>
              <th>Assignee</th>
              <th>Due</th>
              <th>Blocked by</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="a in openActions" :key="a.id">
              <td class="ref">{{ a.reference }}</td>
              <td class="whitespace-pre-line">{{ a.description }}</td>
              <td>
                <span v-if="a.assignee">{{ a.assignee.display_name }}</span>
                <span v-else data-unassigned="true" class="unassigned">Unassigned</span>
              </td>
              <td>
                <template v-if="a.due_date">
                  <span
                    :data-overdue="String(isOverdue(a, generated))"
                    :class="{ overdue: isOverdue(a, generated) }"
                    >{{ a.due_date }}</span
                  >
                  <span v-if="isOverdue(a, generated)" class="overdue-note">▲ Overdue</span>
                </template>
              </td>
              <td class="ref blocked">{{ refs(a.blocked_by) }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section>
        <h2>
          <span id="report-blockers">Open Blockers</span>
          <span class="count">{{ openBlockers.length }}</span>
        </h2>
        <table aria-labelledby="report-blockers">
          <colgroup>
            <col class="w-ref" />
            <col />
            <col class="w-people" />
            <col class="w-refs" />
          </colgroup>
          <thead>
            <tr>
              <th>Ref</th>
              <th>Description</th>
              <th>Accountable</th>
              <th>Blocks</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="b in openBlockers" :key="b.id">
              <td class="ref">{{ b.reference }}</td>
              <td class="whitespace-pre-line">{{ b.description }}</td>
              <td>{{ b.accountable?.display_name ?? '' }}</td>
              <td class="ref">{{ refs(b.blocked_actions) }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section>
        <h2>
          <span id="report-risks">Open Risks</span>
          <span class="count">{{ openRisks.length }}</span>
        </h2>
        <table aria-labelledby="report-risks">
          <colgroup>
            <col class="w-ref" />
            <col />
            <col class="w-li" />
            <col class="w-li" />
            <col class="w-score" />
          </colgroup>
          <thead>
            <tr>
              <th>Ref</th>
              <th>Description</th>
              <th>L</th>
              <th>I</th>
              <th>Score</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in openRisks" :key="r.id">
              <td class="ref">{{ r.reference }}</td>
              <td class="whitespace-pre-line">{{ r.description }}</td>
              <td>{{ r.likelihood }}</td>
              <td>{{ r.impact }}</td>
              <td>
                <span
                  class="severity"
                  :class="TONE_TEXT[SEVERITY[severity(r.score)].tone]"
                  :data-severity="severity(r.score)"
                  >{{ tag(SEVERITY[severity(r.score)]) }} {{ r.score }}</span
                >
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      <footer class="colophon">
        <span>Lodestar</span>
        <span>{{ project.data.value.project_code }} · {{ generated }}</span>
      </footer>
    </template>
  </article>
</template>

<style scoped>
/* Ledger layout (#51): a booktabs table, Inter in points, colour on text and strokes only. */
.report {
  font-family: var(--font-sans);
  font-size: 10pt;
  line-height: 1.45;
  color: var(--color-text);
}
h1 {
  font-size: 20pt;
}
h2 {
  font-size: 12.5pt;
  margin-bottom: 1.5mm;
}
.label,
.meta,
.colophon,
th {
  font-size: 8pt;
  color: var(--color-muted);
}
.label,
th {
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-weight: 550;
}
.cell,
td {
  font-size: 9pt;
}
.count {
  margin-left: 0.4em;
  font-weight: 400;
  color: var(--color-muted);
}
.masthead {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: 4mm;
  padding-bottom: 3mm;
  border-bottom: 1.5pt solid var(--color-text);
}
.pill {
  display: inline-block;
  padding: 0.5mm 3mm;
  border: 1.5pt solid currentColor;
  border-radius: 999px;
  font-weight: 600;
}
.meta {
  margin-top: 1mm;
}
.comment {
  margin-top: 3mm;
  padding-left: 3mm;
  border-left: 2pt solid var(--color-border-strong);
  font-style: italic;
  max-width: 65ch;
}
.prose {
  margin-top: 3mm;
  max-width: 65ch;
}
section {
  margin-top: 7mm;
}
table {
  width: 100%;
  table-layout: fixed;
  border-collapse: collapse;
  border-bottom: 0.75pt solid var(--color-text);
}
thead tr {
  border-bottom: 0.75pt solid var(--color-text);
}
tbody tr + tr {
  border-top: 0.4pt solid var(--color-border);
}
tr {
  break-inside: avoid;
}
th,
td {
  text-align: left;
  vertical-align: top;
  padding: 2mm 3mm 2mm 0;
}
.w-ref {
  width: 24mm;
}
.w-people {
  width: 30mm;
}
.w-due {
  width: 28mm;
}
.w-refs {
  width: 24mm;
}
.w-li {
  width: 10mm;
}
.w-score {
  width: 26mm;
}
.ref {
  font-family: var(--font-mono);
  font-size: 0.92em;
  color: var(--color-muted);
}
.blocked {
  color: var(--color-st-red-fg);
  font-weight: 550;
}
.overdue {
  font-weight: 700;
  color: var(--color-st-red-fg);
}
.overdue-note {
  display: block;
  font-size: 8pt;
  color: var(--color-st-red-fg);
}
.unassigned {
  font-style: italic;
  color: var(--color-st-amber-fg);
}
.severity {
  font-weight: 600;
}
.colophon {
  display: flex;
  justify-content: space-between;
  margin-top: 7mm;
  padding-top: 1.5mm;
  border-top: 0.4pt solid var(--color-border);
}
</style>
