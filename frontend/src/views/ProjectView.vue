<script setup lang="ts">
import { toRef } from 'vue'
import HealthRatingControl from '../components/HealthRatingControl.vue'
import { useProject } from '../projects'

const props = defineProps<{ id: string }>()
const project = useProject(toRef(props, 'id'))
</script>

<template>
  <section>
    <p v-if="project.isPending.value" class="text-gray-600">Loading…</p>
    <p v-else-if="project.error.value" role="alert" class="text-red-700">
      Couldn't load the Project: {{ project.error.value.message }}
    </p>
    <header v-else-if="project.data.value" class="grid gap-2">
      <h1 class="text-2xl font-semibold">
        <span class="mr-2 font-mono text-gray-600">{{ project.data.value.project_code }}</span>
        {{ project.data.value.name }}
      </h1>
      <HealthRatingControl :project="project.data.value" />
    </header>
  </section>
</template>
