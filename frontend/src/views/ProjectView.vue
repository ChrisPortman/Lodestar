<script setup lang="ts">
import { computed, onBeforeUnmount, ref, toRef, watch, watchEffect } from 'vue'
import { RouterLink } from 'vue-router'
import ActionsPanel from '../components/ActionsPanel.vue'
import BlockersPanel from '../components/BlockersPanel.vue'
import CommentsSection from '../components/CommentsSection.vue'
import HealthRatingControl from '../components/HealthRatingControl.vue'
import ProjectMenuPanel from '../components/ProjectMenuPanel.vue'
import MeetingsVariant from '../components/meetings/MeetingsVariant.prototype.vue'
import PrototypeSwitcher from '../components/meetings/PrototypeSwitcher.prototype.vue'
import { stub, useVariant } from '../components/meetings/meetingStub.prototype'
import RisksPanel from '../components/RisksPanel.vue'
import { markViewed, provideComments, useComments } from '../comments'
import { useMembers } from '../members'
import { useProject } from '../projects'
import { useSession } from '../session'

const props = defineProps<{ id: string }>()
const projectId = toRef(props, 'id')
const project = useProject(projectId)
const session = useSession()
const members = useMembers(projectId)
const comments = useComments(projectId)
const showMenu = ref(false)
const variant = useVariant()

// Highlighting compares against the previous visit for the whole page visit.
const since = ref<string | null>(null)
watch(projectId, (id) => (since.value = markViewed(id)), { immediate: true })
// PROTOTYPE (#75) D: make room for the rail on the whole page, so the centred
// content keeps its width and only narrows when the window is too small.
watchEffect(() => (document.body.style.marginRight = variant.value === 'D' && stub.running ? (stub.railCollapsed ? '3rem' : '24rem') : ''))
onBeforeUnmount(() => (document.body.style.marginRight = ''))

const actingId = computed(() => session.data.value?.person.id ?? null)
provideComments({
  projectId,
  comments: computed(() => comments.data.value ?? []),
  since,
  actingId,
  canComment: computed(
    () =>
      !project.data.value?.archived &&
      (members.data.value ?? []).some((m) => m.id === actingId.value),
  ),
})
</script>

<template>
  <section :class="{ 'pr-96': variant === 'B' && (stub.running || stub.justEnded) }">
    <PrototypeSwitcher />
    <MeetingsVariant v-if="project.data.value" :project="project.data.value" place="page" />
    <p v-if="project.isPending.value" class="text-muted">Loading…</p>
    <p v-else-if="project.error.value" role="alert" class="text-danger-text">
      Couldn't load the Project: {{ project.error.value.message }}
    </p>
    <template v-else-if="project.data.value">
      <header class="grid gap-2">
        <div class="flex flex-wrap items-center gap-3">
          <h1 class="flex-1 text-2xl font-semibold">
            <span class="mr-2 font-mono text-muted">{{ project.data.value.project_code }}</span>
            {{ project.data.value.name }}
          </h1>
          <span
            v-if="!project.data.value.project_leader"
            class="rounded bg-st-amber-bg px-2 py-0.5 text-sm text-st-amber-fg"
          >
            No Project Leader
          </span>
          <span v-if="session.data.value" class="text-sm text-muted">
            Acting as <strong>{{ session.data.value.person.display_name }}</strong>
          </span>
          <RouterLink :to="`/projects/${id}/report`" class="btn"> Project on a Page </RouterLink>
          <button
            type="button"
            class="btn"
            aria-label="Project menu"
            :aria-expanded="showMenu"
            @click="showMenu = !showMenu"
          >
            <svg viewBox="0 0 24 24" class="size-5" aria-hidden="true">
              <path
                d="M4 6h16M4 12h16M4 18h16"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
              />
            </svg>
          </button>
        </div>
        <HealthRatingControl :project="project.data.value" />
      </header>
      <ProjectMenuPanel v-if="showMenu" :project="project.data.value" @close="showMenu = false" />
      <div class="mt-6 grid gap-6">
        <!-- Panels in FR-94 order: Actions, Blockers, then Risks last. -->
        <ActionsPanel :project="project.data.value" />
        <BlockersPanel :project="project.data.value" />
        <RisksPanel :project="project.data.value" />
        <section
          aria-labelledby="project-comments"
          class="rounded border border-border bg-surface p-4"
        >
          <h2 id="project-comments" class="text-lg font-semibold">Project Comments</h2>
          <p v-if="comments.error.value" role="alert" class="text-sm text-danger-text">
            Couldn't load Comments: {{ comments.error.value.message }}
          </p>
          <CommentsSection target-type="project" :target-id="id" label="the Project" />
        </section>
      </div>
    </template>
  </section>
</template>
