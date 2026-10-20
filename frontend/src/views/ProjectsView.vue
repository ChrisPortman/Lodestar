<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import type { Project } from '../api/client'
import { suggestProjectCode } from '../api/projects'
import { useCreateProject, useProjects, useSetArchived, useUpdateProject } from '../projects'

const showArchived = ref(false)
const projects = useProjects(showArchived)
const create = useCreateProject()
const update = useUpdateProject()
const archive = useSetArchived()

/** `null` for an empty or blank description, which the API stores as none. */
function optionalText(text: string): string | null {
  return text.trim() || null
}

// --- New Project dialog ---
const creating = ref(false)
const newName = ref('')
const newCode = ref('')
const newDescription = ref('')
const codeEdited = ref(false)
let suggestion = 0
let suggestTimer: ReturnType<typeof setTimeout> | undefined

function openCreate() {
  create.reset()
  newName.value = ''
  newCode.value = ''
  newDescription.value = ''
  codeEdited.value = false
  creating.value = true
  nextTick(() => document.getElementById('new-project-name')?.focus())
}

// Suggest a code as the name is typed, until the code is edited by hand (FR-11).
watch(newName, (name) => {
  if (codeEdited.value) return
  clearTimeout(suggestTimer)
  const request = ++suggestion
  suggestTimer = setTimeout(async () => {
    const code = name.trim() ? await suggestProjectCode(name).catch(() => null) : null
    if (request === suggestion && !codeEdited.value) newCode.value = code ?? ''
  }, 150)
})

// Codes are uppercase (FR-17); typing in lowercase is forgiven.
watch(newCode, (code) => {
  if (code !== code.toUpperCase()) newCode.value = code.toUpperCase()
})

async function submitCreate() {
  const name = newName.value.trim()
  if (!name || !newCode.value) return
  await create.mutateAsync({
    name,
    projectCode: newCode.value,
    description: optionalText(newDescription.value),
  })
  creating.value = false
}

// --- Editing ---
const editingId = ref<string | null>(null)
const draftName = ref('')
const draftDescription = ref('')

async function startEditing(project: Project) {
  update.reset()
  editingId.value = project.id
  draftName.value = project.name
  draftDescription.value = project.description ?? ''
  await nextTick()
  document.getElementById(`name-${project.id}`)?.focus()
}

async function save(project: Project) {
  const name = draftName.value.trim()
  if (!name) return
  await update.mutateAsync({ project, name, description: optionalText(draftDescription.value) })
  editingId.value = null
}

function toggleArchived(project: Project) {
  archive.mutate({ project, archived: !project.archived })
}
</script>

<template>
  <section>
    <div class="flex items-center gap-4">
      <h1 class="flex-1 text-2xl font-semibold">Projects</h1>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="showArchived" type="checkbox" />
        Show archived
      </label>
      <button type="button" class="btn btn-primary btn-lg" @click="openCreate">New Project</button>
    </div>

    <div
      v-if="creating"
      role="dialog"
      aria-labelledby="new-project-title"
      class="mt-4 max-w-xl rounded border border-control-border bg-surface p-4 shadow"
    >
      <h2 id="new-project-title" class="text-lg font-semibold">New Project</h2>
      <form class="mt-2 grid gap-3" @submit.prevent="submitCreate().catch(() => {})">
        <label class="grid gap-1">
          <span class="text-sm font-medium">Name</span>
          <input
            id="new-project-name"
            v-model="newName"
            required
            maxlength="100"
            class="field px-3 py-2"
          />
        </label>
        <label class="grid gap-1">
          <span class="text-sm font-medium">Project Code</span>
          <input
            v-model="newCode"
            required
            maxlength="10"
            pattern="[A-Z][A-Z0-9]{1,9}"
            title="An uppercase letter followed by 1–9 uppercase letters or digits"
            class="field px-3 py-2 font-mono uppercase"
            aria-describedby="new-project-code-hint"
            @input="codeEdited = true"
          />
        </label>
        <p id="new-project-code-hint" class="-mt-2 text-xs text-muted">
          Suggested from the name. It can't be changed later.
        </p>
        <label class="grid gap-1">
          <span class="text-sm font-medium">Description</span>
          <textarea v-model="newDescription" maxlength="4000" rows="3" class="field px-3 py-2" />
        </label>
        <p v-if="create.error.value" role="alert" class="text-danger-text">
          Couldn't create: {{ create.error.value.message }}
        </p>
        <div class="flex gap-2">
          <button type="submit" class="btn btn-primary btn-lg">Create</button>
          <button type="button" class="btn btn-lg" @click="creating = false">Cancel</button>
        </div>
      </form>
    </div>

    <p v-if="projects.isPending.value" class="mt-4 text-muted">Loading…</p>
    <ul v-else aria-label="Projects" class="mt-4 divide-y divide-divider">
      <li v-for="project in projects.data.value" :key="project.id" class="flex gap-3 py-3">
        <span class="w-24 shrink-0 font-mono text-sm text-muted">{{ project.project_code }}</span>
        <form
          v-if="editingId === project.id"
          class="grid flex-1 gap-2"
          @submit.prevent="save(project).catch(() => {})"
        >
          <label class="sr-only" :for="`name-${project.id}`">Name for {{ project.name }}</label>
          <input :id="`name-${project.id}`" v-model="draftName" maxlength="100" class="field" />
          <label class="sr-only" :for="`description-${project.id}`">
            Description for {{ project.name }}
          </label>
          <textarea
            :id="`description-${project.id}`"
            v-model="draftDescription"
            maxlength="4000"
            rows="2"
            class="field"
          />
          <div class="flex gap-2">
            <button type="submit" class="btn btn-primary">Save</button>
            <button type="button" class="btn" @click="editingId = null">Cancel</button>
          </div>
        </form>
        <template v-else>
          <div class="flex-1">
            <RouterLink :to="`/projects/${project.id}`" class="font-medium hover:underline">
              {{ project.name }}
            </RouterLink>
            <span
              v-if="project.archived"
              class="ml-2 rounded bg-active px-2 py-0.5 text-xs text-muted"
            >
              Archived
            </span>
            <p v-if="project.description" class="text-sm text-muted">
              {{ project.description }}
            </p>
            <p class="text-xs text-muted">
              Leader: {{ project.project_leader?.display_name ?? 'none' }}
            </p>
          </div>
          <div class="flex items-start gap-2">
            <button
              v-if="!project.archived"
              type="button"
              class="btn"
              :aria-label="`Edit ${project.name}`"
              @click="startEditing(project)"
            >
              Edit
            </button>
            <button
              type="button"
              class="btn"
              :aria-label="`${project.archived ? 'Unarchive' : 'Archive'} ${project.name}`"
              @click="toggleArchived(project)"
            >
              {{ project.archived ? 'Unarchive' : 'Archive' }}
            </button>
          </div>
        </template>
      </li>
    </ul>
    <p v-if="update.error.value" role="alert" class="mt-2 text-danger-text">
      Couldn't save: {{ update.error.value.message }}
    </p>
    <p v-if="archive.error.value" role="alert" class="mt-2 text-danger-text">
      Couldn't change archiving: {{ archive.error.value.message }}
    </p>
  </section>
</template>
