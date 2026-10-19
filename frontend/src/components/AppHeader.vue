<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { usePeople, useSession, useStartSession } from '../session'

const session = useSession()
const people = usePeople()
const start = useStartSession()

const acting = computed(() => session.data.value?.person)

function switchTo(event: Event) {
  const id = (event.target as HTMLSelectElement).value
  if (id && id !== acting.value?.id) start.mutate(id)
}
</script>

<template>
  <header class="border-b border-gray-200 bg-white print:hidden">
    <div class="mx-auto flex max-w-5xl items-center gap-6 px-4 py-3">
      <RouterLink to="/projects" class="text-lg font-semibold">Lodestar</RouterLink>
      <nav v-if="acting" class="flex gap-4 text-sm">
        <RouterLink to="/projects" class="hover:underline">Projects</RouterLink>
        <RouterLink to="/people" class="hover:underline">People</RouterLink>
      </nav>
      <div v-if="acting" class="ml-auto flex items-center gap-2 text-sm">
        <span
          >Acting as <strong data-testid="acting-person">{{ acting.display_name }}</strong></span
        >
        <label class="sr-only" for="switch-person">Switch Acting Person</label>
        <select
          id="switch-person"
          class="rounded border border-gray-300 px-2 py-1"
          :value="acting.id"
          @change="switchTo"
        >
          <option v-for="person in people.data.value" :key="person.id" :value="person.id">
            {{ person.display_name }}
          </option>
        </select>
      </div>
    </div>
  </header>
</template>
