<script setup lang="ts">
import { ref, toRef } from 'vue'
import { RouterLink } from 'vue-router'
import ActionsPanel from '../components/ActionsPanel.vue'
import BlockersPanel from '../components/BlockersPanel.vue'
import HealthRatingControl from '../components/HealthRatingControl.vue'
import MembersPanel from '../components/MembersPanel.vue'
import RisksPanel from '../components/RisksPanel.vue'
import { useProject } from '../projects'
import { useSession } from '../session'

const props = defineProps<{ id: string }>()
const project = useProject(toRef(props, 'id'))
const session = useSession()
const showMembers = ref(false)
</script>

<template>
  <section>
    <p v-if="project.isPending.value" class="text-gray-600">Loading…</p>
    <p v-else-if="project.error.value" role="alert" class="text-red-700">
      Couldn't load the Project: {{ project.error.value.message }}
    </p>
    <template v-else-if="project.data.value">
      <header class="grid gap-2">
        <div class="flex flex-wrap items-center gap-3">
          <h1 class="flex-1 text-2xl font-semibold">
            <span class="mr-2 font-mono text-gray-600">{{ project.data.value.project_code }}</span>
            {{ project.data.value.name }}
          </h1>
          <span
            v-if="!project.data.value.project_leader"
            class="rounded bg-amber-100 px-2 py-0.5 text-sm text-amber-900"
          >
            No Project Leader
          </span>
          <span v-if="session.data.value" class="text-sm text-gray-600">
            Acting as <strong>{{ session.data.value.person.display_name }}</strong>
          </span>
          <RouterLink
            :to="`/projects/${id}/report`"
            class="rounded border border-gray-300 px-3 py-1 text-sm hover:bg-gray-100"
          >
            Project on a Page
          </RouterLink>
          <button
            type="button"
            class="rounded border border-gray-300 px-3 py-1 text-sm hover:bg-gray-100"
            :aria-expanded="showMembers"
            @click="showMembers = !showMembers"
          >
            Members
          </button>
        </div>
        <HealthRatingControl :project="project.data.value" />
      </header>
      <MembersPanel v-if="showMembers" :project="project.data.value" @close="showMembers = false" />
      <div class="mt-6 grid gap-6">
        <!-- Panels in FR-94 order: Actions, Blockers, then Risks last. -->
        <ActionsPanel :project="project.data.value" />
        <BlockersPanel :project="project.data.value" />
        <RisksPanel :project="project.data.value" />
      </div>
    </template>
  </section>
</template>
