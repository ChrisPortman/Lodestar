<script setup lang="ts">
import { computed, ref } from 'vue'
import type { components } from '../api/schema'

const props = defineProps<{ project: components['schemas']['Project'] }>()
const includeHistory = ref(false)
// A plain link: the browser sends the session cookie and saves the
// attachment under the name the server gives it (FR-110).
const href = computed(
  () =>
    `/api/v1/projects/${encodeURIComponent(props.project.id)}/export` +
    (includeHistory.value ? '?include_history=true' : ''),
)
</script>

<template>
  <div class="grid gap-4 text-sm">
    <label class="flex items-center gap-2 text-muted">
      <input v-model="includeHistory" type="checkbox" />
      Include history
    </label>
    <a :href="href" download class="btn btn-primary w-4/5 justify-self-center text-center">
      Export
    </a>
  </div>
</template>
