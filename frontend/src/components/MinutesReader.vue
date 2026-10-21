<script setup lang="ts">
// The Minutes reader (#84): a wide overlay over the page that leaves the
// Project menu open underneath. Close and Esc close the reader only.
import { computed, onMounted, ref, useTemplateRef } from 'vue'
import type { MeetingSummary } from '../api/meetings'
import {
  copyMinutes,
  meetingStart,
  minutesParts,
  minutesWhen,
  useMinutesDocument,
  useMinutesText,
} from '../meetings'

const props = defineProps<{ meeting: MeetingSummary }>()
const emit = defineEmits<{ close: [] }>()

const type = ref<'formatted' | 'plain'>('formatted')
const meetingId = computed(() => props.meeting.id)
// The Formatted preview is laid out here from the Minutes document, not the
// server's HTML: that is for email, and its inline styles break the CSP.
const doc = useMinutesDocument(meetingId)
const text = useMinutesText(
  computed(() => props.meeting),
  computed(() => type.value === 'plain'),
)
const parts = computed(() => (doc.data.value ? minutesParts(doc.data.value) : []))
const loadError = computed(() => (type.value === 'formatted' ? doc.error.value : text.error.value))
const tabs = [
  { type: 'formatted', label: 'Formatted' },
  { type: 'plain', label: 'Plain text' },
] as const

const copied = ref('')
const copyError = ref('')
async function copy(formatted: boolean) {
  copied.value = copyError.value = ''
  try {
    await copyMinutes(props.meeting, formatted)
    copied.value = formatted ? 'Copied for email or documents' : 'Copied as plain text'
  } catch (e) {
    copyError.value = e instanceof Error ? e.message : String(e)
  }
}

const dialog = useTemplateRef('dialog')
onMounted(() => dialog.value?.focus())
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-20 overflow-y-auto bg-black/40 p-4 sm:p-8"
      @click.self="emit('close')"
    >
      <div
        ref="dialog"
        role="dialog"
        aria-labelledby="minutes-title"
        tabindex="-1"
        class="mx-auto grid max-w-3xl gap-3 rounded bg-surface p-6 shadow-xl"
        @keydown.esc.stop="emit('close')"
      >
        <div class="flex items-center gap-2">
          <h2 id="minutes-title" class="flex-1 text-lg font-semibold">
            Minutes · {{ meetingStart(meeting) }}
          </h2>
          <button type="button" class="btn" @click="emit('close')">Close</button>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <div
            role="tablist"
            aria-label="Preview"
            class="flex rounded border border-control-border text-sm"
          >
            <button
              v-for="tab in tabs"
              :key="tab.type"
              role="tab"
              type="button"
              class="px-2 py-1"
              :class="type === tab.type ? 'bg-accent text-on-accent' : ''"
              :aria-selected="type === tab.type"
              @click="type = tab.type"
            >
              {{ tab.label }}
            </button>
          </div>
          <span class="flex-1" />
          <button type="button" class="btn btn-sm btn-primary" @click="copy(true)">
            Copy formatted
          </button>
          <button type="button" class="btn btn-sm" @click="copy(false)">Copy plain text</button>
        </div>
        <p v-if="copied" role="status" class="text-sm text-st-green-fg">{{ copied }}</p>
        <p v-if="copyError" role="alert" class="text-danger-text">Couldn't copy: {{ copyError }}</p>
        <p v-if="loadError" role="alert" class="text-danger-text">
          Couldn't load the Minutes: {{ loadError.message }}
        </p>
        <article
          v-else-if="type === 'formatted'"
          aria-label="Formatted Minutes"
          class="overflow-x-auto rounded border border-border bg-white p-3 text-sm text-black"
        >
          <template v-if="doc.data.value">
            <h3 class="text-xl font-semibold">{{ doc.data.value.project_name }}</h3>
            <p class="text-neutral-600">{{ minutesWhen(doc.data.value) }}</p>
            <p class="mb-3">
              Led by <b>{{ doc.data.value.leader }}</b
              >. Attendees:
              {{ doc.data.value.attendees.join(', ') }}
            </p>
            <section v-for="part in parts" :key="part.name">
              <h4 class="mt-4 mb-1 text-base font-semibold">{{ part.name }}s</h4>
              <template v-if="part.new.length">
                <h5 class="mt-2 mb-1 font-semibold">New {{ part.name }}s</h5>
                <ul class="mb-2 list-disc pl-5">
                  <li v-for="item in part.new" :key="item.reference">
                    <b>{{ item.reference }}</b
                    >: {{ item.title
                    }}<template v-if="item.state?.length"> ({{ item.state.join(', ') }})</template>
                    <ul v-if="item.comments?.length" class="list-disc pl-5">
                      <li v-for="(c, i) in item.comments" :key="i">
                        <i>{{ c }}</i>
                      </li>
                    </ul>
                  </li>
                </ul>
              </template>
              <template v-if="part.updated.length">
                <h5 class="mt-2 mb-1 font-semibold">Updated {{ part.name }}s</h5>
                <ul class="mb-2 list-disc pl-5">
                  <li v-for="item in part.updated" :key="item.reference">
                    <b>{{ item.reference }}</b
                    >: {{ item.title }}
                    <ul class="list-disc pl-5">
                      <li v-for="(change, i) in item.changes ?? []" :key="i">{{ change }}</li>
                      <li v-for="(c, i) in item.comments ?? []" :key="`c${i}`">
                        <i>{{ c }}</i>
                      </li>
                    </ul>
                  </li>
                </ul>
              </template>
            </section>
            <section v-if="doc.data.value.meeting_comments?.length">
              <h4 class="mt-4 mb-1 text-base font-semibold">Meeting Comments</h4>
              <ul class="mb-2 list-disc pl-5">
                <li v-for="(c, i) in doc.data.value.meeting_comments" :key="i">
                  <i>{{ c }}</i>
                </li>
              </ul>
            </section>
          </template>
        </article>
        <pre
          v-else
          class="overflow-x-auto rounded border border-border bg-subtle p-3 text-xs whitespace-pre-wrap"
          >{{ text.data.value ?? '' }}</pre>
      </div>
    </div>
  </Teleport>
</template>
