<script setup lang="ts">
// PROTOTYPE (#75): floating variant switcher plus stub controls. Throwaway.
import { onBeforeUnmount, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { simulateReload, stub, useVariant, VARIANTS, type Variant } from './meetingStub.prototype'

const router = useRouter()
const variant = useVariant()
const keys = Object.keys(VARIANTS) as Variant[]
const dev = import.meta.env.DEV

function go(step: number) {
  const i = (keys.indexOf(variant.value) + step + keys.length) % keys.length
  router.replace({ query: { ...router.currentRoute.value.query, variant: keys[i] } })
}

function onKey(e: KeyboardEvent) {
  const t = e.target as HTMLElement
  if (t.closest('input, textarea, select, [contenteditable]')) return
  if (e.key === 'ArrowLeft') go(-1)
  if (e.key === 'ArrowRight') go(1)
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div
    v-if="dev"
    class="fixed bottom-4 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-full bg-black px-4 py-2 text-sm text-white shadow-xl"
  >
    <button type="button" aria-label="Previous variant" @click="go(-1)">◀</button>
    <span class="font-mono">{{ variant }} ({{ VARIANTS[variant] }})</span>
    <button type="button" aria-label="Next variant" @click="go(1)">▶</button>
    <span class="opacity-50">|</span>
    <select v-model="stub.role" class="rounded bg-neutral-800 px-1 text-white">
      <option value="leader">Acting: Project Leader</option>
      <option value="member">Acting: Member</option>
      <option value="other-leader-running">Another leader is running one</option>
    </select>
    <button
      type="button"
      class="rounded bg-neutral-700 px-2 disabled:opacity-40"
      :disabled="!stub.running"
      @click="simulateReload"
    >
      Simulate reload
    </button>
  </div>
</template>
