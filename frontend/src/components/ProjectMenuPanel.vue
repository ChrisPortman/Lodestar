<script setup lang="ts">
import { ref } from 'vue'
import type { Project } from '../api/client'
import ExportControl from './ExportControl.vue'
import MembersPanel from './MembersPanel.vue'

defineProps<{ project: Project }>()
const emit = defineEmits<{ close: [] }>()

// Always opens on the menu: the panel is remounted on each open (#61).
const view = ref<'menu' | 'members' | 'export'>('menu')
const titles = { menu: 'Project menu', members: 'Members', export: 'Export' }
</script>

<template>
  <aside
    role="dialog"
    aria-labelledby="project-menu-title"
    class="fixed inset-y-0 right-0 z-10 w-full max-w-sm overflow-y-auto border-l border-control-border bg-surface p-4 shadow-lg"
    @keydown.esc="emit('close')"
  >
    <div class="flex items-center gap-2">
      <button v-if="view !== 'menu'" type="button" class="btn" @click="view = 'menu'">Back</button>
      <h2 id="project-menu-title" class="flex-1 text-lg font-semibold">{{ titles[view] }}</h2>
      <button type="button" class="btn" @click="emit('close')">Close</button>
    </div>

    <nav v-if="view === 'menu'" class="mt-4 grid gap-2">
      <button type="button" class="btn" @click="view = 'members'">Manage Members</button>
      <button type="button" class="btn" @click="view = 'export'">Export</button>
    </nav>
    <MembersPanel v-else-if="view === 'members'" :project="project" />
    <ExportControl v-else :project="project" class="mt-4" />
  </aside>
</template>
