<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { NEW_PERSON, usePeople, useSession, useStartSession } from '../session'
import type { PersonRef } from '../api/members'
import NewPersonField from './NewPersonField.vue'
import { useTheme } from '../theme'

const session = useSession()
const people = usePeople()
const start = useStartSession()
const { theme, toggle } = useTheme()

const acting = computed(() => session.data.value?.person)

const creating = ref(false)

function switchTo(event: Event) {
  const id = (event.target as HTMLSelectElement).value
  if (id === NEW_PERSON) {
    creating.value = true
    return
  }
  if (id && id !== acting.value?.id) start.mutate(id)
}

async function switchToNew(person: PersonRef) {
  await start.mutateAsync(person.id)
  creating.value = false
}
</script>

<template>
  <header class="border-b border-border bg-chrome text-chrome-text print:hidden">
    <div class="mx-auto flex max-w-5xl items-center gap-6 px-4 py-3">
      <RouterLink to="/projects" class="text-lg font-semibold">Lodestar</RouterLink>
      <nav v-if="acting" class="flex gap-4 text-sm">
        <RouterLink to="/projects" class="hover:underline">Projects</RouterLink>
        <RouterLink to="/people" class="hover:underline">People</RouterLink>
      </nav>
      <div class="ml-auto flex items-center gap-4 text-sm">
        <div v-if="acting" class="flex items-center gap-2">
          <span
            >Acting as <strong data-testid="acting-person">{{ acting.display_name }}</strong></span
          >
          <label class="sr-only" for="switch-person">Switch Acting Person</label>
          <NewPersonField
            v-if="creating"
            label="New Acting Person"
            @picked="(person) => switchToNew(person).catch(() => {})"
            @cancel="creating = false"
          />
          <select
            v-else
            id="switch-person"
            class="field text-sm"
            :value="acting.id"
            @change="switchTo"
          >
            <option v-for="person in people.data.value" :key="person.id" :value="person.id">
              {{ person.display_name }}
            </option>
            <option :value="NEW_PERSON">New Person…</option>
          </select>
        </div>
        <button type="button" class="btn" :aria-pressed="theme === 'dark'" @click="toggle">
          Dark mode
        </button>
      </div>
    </div>
  </header>
</template>
