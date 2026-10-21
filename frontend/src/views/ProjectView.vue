<script setup lang="ts">
import { computed, ref, toRef, watch } from 'vue'
import { RouterLink } from 'vue-router'
import ActionsPanel from '../components/ActionsPanel.vue'
import BlockersPanel from '../components/BlockersPanel.vue'
import CommentsSection from '../components/CommentsSection.vue'
import HealthRatingControl from '../components/HealthRatingControl.vue'
import MeetingPanel from '../components/MeetingPanel.vue'
import MinutesReader from '../components/MinutesReader.vue'
import ProjectMenuPanel from '../components/ProjectMenuPanel.vue'
import RisksPanel from '../components/RisksPanel.vue'
import { clockTime, useBlockingLock } from '../meetingLock'
import { markViewed, provideComments, useComments } from '../comments'
import { useMembers } from '../members'
import { useProject } from '../projects'
import { provideRunningMeeting } from '../runningMeeting'
import { useSession } from '../session'

const props = defineProps<{ id: string }>()
const projectId = toRef(props, 'id')
const project = useProject(projectId)
const session = useSession()
const members = useMembers(projectId)
const comments = useComments(projectId)
const showMenu = ref(false)
const blockingLock = useBlockingLock(projectId)
const meeting = provideRunningMeeting(projectId)

// The Meeting panel and the Project menu are never open together.
watch(meeting.panelOpen, (open) => open && (showMenu.value = false))
function toggleMenu() {
  showMenu.value = !showMenu.value
  if (showMenu.value) meeting.panelOpen.value = false
}

// Highlighting compares against the previous visit for the whole page visit.
const since = ref<string | null>(null)
watch(projectId, (id) => (since.value = markViewed(id)), { immediate: true })

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
  <!-- While a Meeting runs the page keeps the strip's width free, so nothing moves. -->
  <section :class="{ 'pr-10': meeting.running.value }">
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
            @click="toggleMenu"
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
        <p
          v-if="blockingLock"
          role="status"
          aria-label="Meeting in progress"
          class="rounded bg-st-amber-bg px-3 py-2 text-st-amber-fg"
        >
          Meeting in progress, led by {{ blockingLock.leader.display_name }}, since
          {{ clockTime(blockingLock.acquired_at) }}
        </p>
        <!-- A disabled fieldset disables every control inside it (Member management is outside). -->
        <fieldset :disabled="!!blockingLock" class="contents">
          <HealthRatingControl :project="project.data.value" />
        </fieldset>
      </header>
      <ProjectMenuPanel v-if="showMenu" :project="project.data.value" @close="showMenu = false" />
      <MeetingPanel v-if="meeting.running.value && meeting.panelOpen.value" :project-id="id" />
      <button
        v-else-if="meeting.running.value"
        type="button"
        class="fixed inset-y-0 right-0 z-10 w-10 border-l-4 border-accent bg-accent-soft text-sm font-medium [writing-mode:vertical-rl]"
        @click="meeting.panelOpen.value = true"
      >
        ● Meeting {{ meeting.elapsed.value }} · {{ meeting.state.comments.length }} Comments
      </button>
      <MinutesReader
        v-if="meeting.created.value"
        :meeting="meeting.created.value"
        @close="meeting.created.value = null"
      />
      <fieldset :disabled="!!blockingLock" class="mt-6 grid min-w-0 gap-6">
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
      </fieldset>
    </template>
  </section>
</template>
