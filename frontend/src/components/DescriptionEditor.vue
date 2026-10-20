<script setup lang="ts">
// A multi-line description with an explicit Save or Cancel (FR-93):
// Ctrl+Enter saves, Esc cancels.
import { onMounted, ref, useTemplateRef } from 'vue'

const props = defineProps<{ initial: string; label: string; saveLabel?: string }>()
const emit = defineEmits<{ save: [text: string]; cancel: [] }>()

const text = ref(props.initial)
// If the stored text changes while editing (a 412 refetch), keep the user's
// text and show the new value alongside it (#8).
const started = props.initial
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
      class="field"
      @keydown.ctrl.enter.prevent="save"
      @keydown.esc.prevent="emit('cancel')"
    />
    <p v-if="initial !== started" role="status" class="text-sm text-st-amber-fg">
      Changed elsewhere since you started editing. Now:
      <span class="whitespace-pre-line">{{ initial }}</span>
    </p>
    <slot />
    <div class="flex gap-2">
      <button type="submit" class="btn btn-primary">
        {{ saveLabel ?? 'Save' }}
      </button>
      <button type="button" class="btn" @click="emit('cancel')">Cancel</button>
    </div>
  </form>
</template>
