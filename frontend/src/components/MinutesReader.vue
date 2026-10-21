<script setup lang="ts">
// The Minutes reader (#84): a wide overlay over the page that leaves the
// Project menu open underneath. Close and Esc close the reader only.
import { computed, onMounted, ref, useTemplateRef } from 'vue'
import type { MeetingSummary, MinutesType } from '../api/meetings'
import { copyMinutes, meetingStart, useMinutes } from '../meetings'

const props = defineProps<{ meeting: MeetingSummary }>()
const emit = defineEmits<{ close: [] }>()

const type = ref<MinutesType>('text/html')
const minutes = useMinutes(
  computed(() => props.meeting.id),
  type,
)
const tabs = [
  { type: 'text/html', label: 'Formatted' },
  { type: 'text/plain', label: 'Plain text' },
] as const

const copied = ref('')
const copyError = ref('')
async function copy(formatted: boolean) {
  copied.value = copyError.value = ''
  try {
    await copyMinutes(props.meeting.id, formatted)
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
        <p v-if="minutes.error.value" role="alert" class="text-danger-text">
          Couldn't load the Minutes: {{ minutes.error.value.message }}
        </p>
        <!-- The server escapes every value in the HTML Minutes (#83). -->
        <!-- eslint-disable vue/no-v-html -->
        <div
          v-else-if="type === 'text/html'"
          class="overflow-x-auto rounded border border-border bg-white p-3 text-sm text-black"
          v-html="minutes.data.value ?? ''"
        />
        <pre
          v-else
          class="overflow-x-auto rounded border border-border bg-subtle p-3 text-xs whitespace-pre-wrap"
          >{{ minutes.data.value ?? '' }}</pre>
        <!-- eslint-enable vue/no-v-html -->
      </div>
    </div>
  </Teleport>
</template>
