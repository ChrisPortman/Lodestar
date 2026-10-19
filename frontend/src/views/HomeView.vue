<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useCreatePerson, usePeople, useSession, useStartSession } from '../session'

const router = useRouter()
const session = useSession()
const people = usePeople()
const start = useStartSession()
const create = useCreatePerson()
const newName = ref('')

// An existing session skips the picker.
watch(
  () => session.data.value,
  (s) => {
    if (s && !start.isPending.value && !create.isPending.value) router.replace('/projects')
  },
  { immediate: true },
)

async function pick(id: string) {
  await start.mutateAsync(id)
  await router.push('/projects')
}

async function createAndPick() {
  const name = newName.value.trim()
  if (!name) return
  await create.mutateAsync(name)
  await router.push('/projects')
}
</script>

<template>
  <section class="mx-auto max-w-md">
    <h1 class="text-2xl font-semibold">Who are you?</h1>
    <p v-if="people.isPending.value" class="mt-4 text-gray-600">Loading…</p>
    <ul v-else class="mt-4 space-y-2">
      <li v-for="person in people.data.value" :key="person.id">
        <button
          type="button"
          class="w-full rounded border border-gray-300 px-4 py-2 text-left hover:bg-gray-100"
          @click="pick(person.id).catch(() => {})"
        >
          {{ person.display_name }}
        </button>
      </li>
    </ul>
    <form class="mt-6 flex gap-2" @submit.prevent="createAndPick().catch(() => {})">
      <label class="sr-only" for="new-person">New Person's name</label>
      <input
        id="new-person"
        v-model="newName"
        maxlength="100"
        placeholder="Your name"
        class="flex-1 rounded border border-gray-300 px-3 py-2"
      />
      <button type="submit" class="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700">
        I'm new
      </button>
    </form>
    <p v-if="start.error.value || create.error.value" role="alert" class="mt-4 text-red-700">
      {{ (start.error.value ?? create.error.value)?.message }}
    </p>
  </section>
</template>
