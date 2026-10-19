<script setup lang="ts">
// A multi-line description with an explicit Save or Cancel (FR-93):
// Ctrl+Enter saves, Esc cancels.
import { onMounted, ref, useTemplateRef } from 'vue'

const props = defineProps<{ initial: string; label: string; saveLabel?: string }>()
const emit = defineEmits<{ save: [text: string]; cancel: [] }>()

const text = ref(props.initial)
const field = useTemplateRef<HTMLTextAreaElement>('field')
onMounted(() => field.value?.focus())

function save() {
  if (text.value.trim()) emit('save', text.value.trim())
}
</script>

<template>
  <form class="grid gap-2" @submit.prevent="save">
    <textarea
      ref="field"
      v-model="text"
      :aria-label="label"
      required
      maxlength="4000"
      rows="3"
      class="rounded border border-gray-300 px-3 py-1"
      @keydown.ctrl.enter.prevent="save"
      @keydown.esc.prevent="emit('cancel')"
    />
    <slot />
    <div class="flex gap-2">
      <button type="submit" class="rounded bg-blue-600 px-3 py-1 text-white hover:bg-blue-700">
        {{ saveLabel ?? 'Save' }}
      </button>
      <button
        type="button"
        class="rounded border border-gray-300 px-3 py-1"
        @click="emit('cancel')"
      >
        Cancel
      </button>
    </div>
  </form>
</template>
