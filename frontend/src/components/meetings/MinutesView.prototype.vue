<script setup lang="ts">
// PROTOTYPE (#75): the Minutes with a preview toggle and two copy buttons.
import { ref } from 'vue'
import { copyMinutes, minutesHtml, minutesText, type PastMeeting } from './meetingStub.prototype'

const props = defineProps<{ meeting: PastMeeting; project: string; wide?: boolean }>()
const as = ref<'html' | 'text'>('html')
const copied = ref('')

async function copy(kind: 'text' | 'html') {
  await copyMinutes(props.meeting, props.project, kind)
  copied.value = kind === 'html' ? 'Copied for email or docs' : 'Copied as plain text'
  setTimeout(() => (copied.value = ''), 2000)
}
</script>

<template>
  <div class="grid gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <div role="tablist" class="flex rounded border border-control-border text-sm">
        <button
          v-for="k in ['html', 'text'] as const"
          :key="k"
          role="tab"
          type="button"
          class="px-2 py-1"
          :class="as === k ? 'bg-accent text-on-accent' : ''"
          :aria-selected="as === k"
          @click="as = k"
        >
          {{ k === 'html' ? 'Formatted' : 'Plain text' }}
        </button>
      </div>
      <span class="flex-1" />
      <button type="button" class="btn btn-sm btn-primary" @click="copy('html')">
        Copy formatted
      </button>
      <button type="button" class="btn btn-sm" @click="copy('text')">Copy plain text</button>
    </div>
    <p v-if="copied" role="status" class="text-sm text-st-green-fg">{{ copied }}</p>
    <!-- eslint-disable-next-line vue/no-v-html -- prototype stub content -->
    <div
      v-if="as === 'html'"
      class="minutes-preview rounded border border-border bg-surface p-3 text-sm"
      :class="wide ? '' : 'max-h-[60vh] overflow-y-auto'"
      v-html="minutesHtml(meeting, project)"
    />
    <pre
      v-else
      class="overflow-x-auto rounded border border-border bg-subtle p-3 text-xs"
      :class="wide ? '' : 'max-h-[60vh]'"
      >{{ minutesText(meeting, project) }}</pre>
  </div>
</template>

<style scoped>
.minutes-preview :deep(h2) {
  font-weight: 600;
  font-size: 1rem;
}
.minutes-preview :deep(h3) {
  font-weight: 600;
  margin-top: 0.75rem;
  text-transform: uppercase;
  font-size: 0.75rem;
  letter-spacing: 0.05em;
}
.minutes-preview :deep(h4) {
  font-style: italic;
  margin-top: 0.25rem;
}
.minutes-preview :deep(ul) {
  list-style: disc;
  padding-left: 1.25rem;
}
</style>
