<script setup lang="ts">
import { ref } from 'vue'
import type { Project } from '../api/client'
import type { HealthRating } from '../api/projects'
import { useSetHealthRating } from '../projects'

type Choice = HealthRating['rating'] | 'none'

const props = defineProps<{ project: Project }>()

const LABELS: Record<Choice, string> = {
  red: 'Red',
  amber: 'Amber',
  green: 'Green',
  none: 'Not rated',
}
const BADGE: Record<Choice, string> = {
  red: 'bg-red-600 text-white',
  amber: 'bg-amber-400 text-gray-900',
  green: 'bg-green-600 text-white',
  none: 'bg-gray-200 text-gray-700',
}
const CHOICES: Choice[] = ['red', 'amber', 'green', 'none']

const save = useSetHealthRating()
const editing = ref(false)
const draftRating = ref<Choice>('none')
const draftComment = ref('')

function current(): Choice {
  return props.project.health_rating?.rating ?? 'none'
}

function startEditing() {
  draftRating.value = current()
  draftComment.value = props.project.health_rating?.comment ?? ''
  save.reset()
  editing.value = true
}

async function submit() {
  const healthRating =
    draftRating.value === 'none'
      ? null
      : { rating: draftRating.value, comment: draftComment.value.trim() || null }
  await save.mutateAsync({ project: props.project, healthRating })
  editing.value = false
}
</script>

<template>
  <div class="grid gap-2">
    <div class="flex items-center gap-2">
      <span
        role="status"
        aria-label="Health Rating"
        class="rounded px-2 py-0.5 text-sm font-medium"
        :class="BADGE[current()]"
      >
        {{ LABELS[current()] }}
      </span>
      <button
        v-if="!project.archived && !editing"
        type="button"
        aria-label="Edit Health Rating"
        class="rounded border border-gray-300 px-2 py-0.5 text-sm hover:bg-gray-100"
        @click="startEditing"
      >
        Edit
      </button>
    </div>
    <p v-if="project.health_rating?.comment && !editing" class="text-sm text-gray-700">
      {{ project.health_rating.comment }}
    </p>
    <form
      v-if="editing"
      role="group"
      aria-label="Health Rating"
      class="grid max-w-md gap-2"
      @submit.prevent="submit().catch(() => {})"
    >
      <div class="flex gap-3">
        <label v-for="choice in CHOICES" :key="choice" class="flex items-center gap-1 text-sm">
          <input v-model="draftRating" type="radio" name="health-rating" :value="choice" />
          {{ LABELS[choice] }}
        </label>
      </div>
      <label class="grid gap-1">
        <span class="text-sm font-medium">Comment</span>
        <textarea
          v-model="draftComment"
          :disabled="draftRating === 'none'"
          maxlength="4000"
          rows="2"
          class="rounded border border-gray-300 px-3 py-1"
        />
      </label>
      <p v-if="save.error.value" role="alert" class="text-red-700">
        Couldn't save: {{ save.error.value.message }}
      </p>
      <div class="flex gap-2">
        <button type="submit" class="rounded bg-blue-600 px-3 py-1 text-white hover:bg-blue-700">
          Save
        </button>
        <button
          type="button"
          class="rounded border border-gray-300 px-3 py-1"
          @click="editing = false"
        >
          Cancel
        </button>
      </div>
    </form>
  </div>
</template>
