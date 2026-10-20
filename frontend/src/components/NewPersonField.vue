<script setup lang="ts">
// The inline field behind a "New Person…" option (#59): Enter creates the
// Person, Esc cancels, and existing People matching the name are offered
// first so People, who are never deleted, aren't duplicated.
import { computed, onMounted, ref } from 'vue'
import type { PersonRef } from '../api/members'
import { useCreatePerson, usePeople } from '../session'

defineProps<{ label: string }>()
const emit = defineEmits<{ picked: [person: PersonRef]; cancel: [] }>()

const people = usePeople()
const create = useCreatePerson()
const name = ref('')
const input = ref<HTMLInputElement>()

onMounted(() => input.value?.focus())

const matches = computed(() => {
  const typed = name.value.trim().toLowerCase()
  if (!typed) return []
  return (people.data.value ?? []).filter((p) => p.display_name.toLowerCase().includes(typed))
})

async function save() {
  const displayName = name.value.trim()
  if (!displayName) return
  const person = await create.mutateAsync(displayName)
  emit('picked', { id: person.id, display_name: person.display_name })
}
</script>

<template>
  <span class="inline-flex flex-col gap-1">
    <input
      ref="input"
      v-model="name"
      :aria-label="label"
      placeholder="New person's name"
      class="field text-sm"
      @keydown.enter.prevent="save().catch(() => {})"
      @keydown.esc.stop.prevent="emit('cancel')"
    />
    <button
      v-for="p in matches"
      :key="p.id"
      type="button"
      class="btn btn-sm text-left"
      @click="emit('picked', { id: p.id, display_name: p.display_name })"
    >
      Use existing {{ p.display_name }}
    </button>
    <span v-if="create.error.value" role="alert" class="text-danger-text">
      Couldn't create Person: {{ create.error.value.message }}
    </span>
  </span>
</template>
