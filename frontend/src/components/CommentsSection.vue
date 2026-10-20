<script setup lang="ts">
// A target's Comments (#42): collapsed by default, oldest first, with a
// "new" badge for Comments since the Project was last viewed.
import { computed, ref, toRef } from 'vue'
import { ApiError } from '../api/client'
import type { Comment, CommentTarget } from '../api/comments'
import {
  useCreateComment,
  useDeleteComment,
  useTargetComments,
  useUpdateComment,
} from '../comments'
import DescriptionEditor from './DescriptionEditor.vue'

const props = defineProps<{
  targetType: CommentTarget
  targetId: string
  /** What the Comments are about, e.g. "R-001", for accessible names. */
  label: string
}>()

const { projectId, comments, actingId, canComment } = useTargetComments(
  props.targetType,
  toRef(props, 'targetId'),
)
const create = useCreateComment()
const update = useUpdateComment()
const remove = useDeleteComment()
const error = computed(() => create.error.value ?? update.error.value ?? remove.error.value)
const hasNew = computed(() => comments.value.some((c) => c.isNew))

const draft = ref('')
const editingId = ref<string | null>(null)
const confirmingId = ref<string | null>(null)

const mine = (comment: Comment) => !comment.deleted && comment.author.id === actingId.value
const when = (iso: string) => new Date(iso).toLocaleString()

async function add() {
  if (!draft.value.trim()) return
  await create.mutateAsync({
    about: { projectId: projectId.value, targetType: props.targetType, targetId: props.targetId },
    body: draft.value,
  })
  draft.value = ''
}

async function save(comment: Comment, body: string) {
  try {
    await update.mutateAsync({ comment, body })
    editingId.value = null
  } catch (e) {
    // Changed elsewhere: close the editor so the refreshed text shows.
    if (e instanceof ApiError && e.status === 412) editingId.value = null
    throw e
  }
}

async function confirmDelete(comment: Comment) {
  confirmingId.value = null
  await remove.mutateAsync(comment)
}
</script>

<template>
  <details class="mt-1 text-sm">
    <summary class="cursor-pointer text-gray-600">
      Comments ({{ comments.length }})
      <span v-if="hasNew" class="ml-1 font-semibold text-blue-700">• new</span>
    </summary>
    <ul :aria-label="`Comments on ${label}`" class="mt-1 grid gap-2">
      <li
        v-for="{ comment, isNew } in comments"
        :key="comment.id"
        :data-new="isNew || undefined"
        class="rounded border-l-2 py-1 pl-2"
        :class="isNew ? 'border-blue-500 bg-blue-50' : 'border-gray-200'"
      >
        <div class="flex flex-wrap items-baseline gap-2 text-xs text-gray-600">
          <strong class="text-gray-800">{{ comment.author.display_name }}</strong>
          <time :datetime="comment.created_at">{{ when(comment.created_at) }}</time>
          <span v-if="comment.edited_at && !comment.deleted" class="italic text-gray-500">
            edited: <time :datetime="comment.edited_at">{{ when(comment.edited_at) }}</time>
          </span>
          <span v-if="isNew" class="font-semibold text-blue-700">new</span>
        </div>
        <p v-if="comment.deleted" class="italic text-gray-500">Comment deleted by author</p>
        <DescriptionEditor
          v-else-if="editingId === comment.id"
          :initial="comment.body"
          label="Edit Comment"
          @save="(text) => save(comment, text).catch(() => {})"
          @cancel="editingId = null"
        />
        <p v-else data-testid="comment-body" class="whitespace-pre-wrap break-words">
          {{ comment.body }}
        </p>
        <div
          v-if="canComment && mine(comment) && editingId !== comment.id"
          class="mt-1 flex gap-2 text-xs"
        >
          <template v-if="confirmingId === comment.id">
            <span>Delete this Comment?</span>
            <button
              type="button"
              class="text-red-700 underline"
              @click="confirmDelete(comment).catch(() => {})"
            >
              Yes, delete
            </button>
            <button type="button" class="underline" @click="confirmingId = null">Keep</button>
          </template>
          <template v-else>
            <button type="button" class="underline" @click="editingId = comment.id">Edit</button>
            <button type="button" class="underline" @click="confirmingId = comment.id">
              Delete
            </button>
          </template>
        </div>
      </li>
    </ul>
    <p v-if="error" role="alert" class="mt-1 text-red-700">Couldn't save: {{ error.message }}</p>
    <form v-if="canComment" class="mt-2 grid gap-1" @submit.prevent="add().catch(() => {})">
      <textarea
        v-model="draft"
        :aria-label="`Add a Comment on ${label}`"
        maxlength="4000"
        rows="2"
        class="rounded border border-gray-300 px-2 py-1"
        @keydown.ctrl.enter.prevent="add().catch(() => {})"
      />
      <div>
        <button type="submit" class="rounded bg-blue-600 px-3 py-1 text-white hover:bg-blue-700">
          Comment
        </button>
      </div>
    </form>
  </details>
</template>
