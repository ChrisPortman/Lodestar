<script setup lang="ts">
import { nextTick, ref } from 'vue'
import type { Person } from '../api/client'
import { useCreatePerson, usePeople, useRenamePerson } from '../session'

const people = usePeople()
const create = useCreatePerson()
const rename = useRenamePerson()

const newName = ref('')
const editingId = ref<string | null>(null)
const draft = ref('')

async function add() {
  const name = newName.value.trim()
  if (!name) return
  await create.mutateAsync(name)
  newName.value = ''
}

async function startEditing(person: Person) {
  rename.reset()
  editingId.value = person.id
  draft.value = person.display_name
  await nextTick()
  document.getElementById(`name-${person.id}`)?.focus()
}

async function save(person: Person) {
  const name = draft.value.trim()
  if (!name) return
  await rename.mutateAsync({ person, displayName: name })
  editingId.value = null
}
</script>

<template>
  <section>
    <h1 class="text-2xl font-semibold">People</h1>

    <p v-if="people.isPending.value" class="mt-4 text-muted">Loading…</p>
    <ul v-else aria-label="People" class="mt-4 divide-y divide-divider">
      <li v-for="person in people.data.value" :key="person.id" class="flex items-center gap-2 py-2">
        <form
          v-if="editingId === person.id"
          class="flex flex-1 gap-2"
          @submit.prevent="save(person).catch(() => {})"
        >
          <label class="sr-only" :for="`name-${person.id}`">
            Display name for {{ person.display_name }}
          </label>
          <input
            :id="`name-${person.id}`"
            v-model="draft"
            maxlength="100"
            class="flex-1 rounded border border-control-border px-3 py-1"
          />
          <button
            type="submit"
            class="rounded bg-accent px-3 py-1 text-on-accent hover:bg-accent-hover"
          >
            Save
          </button>
          <button
            type="button"
            class="rounded border border-control-border px-3 py-1"
            @click="editingId = null"
          >
            Cancel
          </button>
        </form>
        <template v-else>
          <span class="flex-1">{{ person.display_name }}</span>
          <button
            type="button"
            class="rounded border border-control-border px-3 py-1 text-sm hover:bg-subtle"
            :aria-label="`Rename ${person.display_name}`"
            @click="startEditing(person)"
          >
            Rename
          </button>
        </template>
      </li>
    </ul>
    <p v-if="rename.error.value" role="alert" class="mt-2 text-danger-text">
      Couldn't rename: {{ rename.error.value.message }}
    </p>

    <form class="mt-6 flex max-w-md gap-2" @submit.prevent="add().catch(() => {})">
      <label class="sr-only" for="new-person">New Person</label>
      <input
        id="new-person"
        v-model="newName"
        maxlength="100"
        placeholder="Display name"
        class="flex-1 rounded border border-control-border px-3 py-2"
      />
      <button
        type="submit"
        class="rounded bg-accent px-4 py-2 text-on-accent hover:bg-accent-hover"
      >
        Add
      </button>
    </form>
    <p v-if="create.error.value" role="alert" class="mt-2 text-danger-text">
      Couldn't add: {{ create.error.value.message }}
    </p>
  </section>
</template>
