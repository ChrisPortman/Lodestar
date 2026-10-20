import { computed, inject, provide, type InjectionKey, type Ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import {
  createComment,
  deleteComment,
  listComments,
  updateComment,
  type Comment,
  type CommentAbout,
  type CommentTarget,
} from './api/comments'

export const commentsKey = ['comments'] as const

export function useComments(projectId: Ref<string>) {
  return useQuery({
    queryKey: computed(() => [...commentsKey, projectId.value]),
    queryFn: () => listComments(projectId.value),
  })
}

/** Refetches on success and failure alike, so a 412 shows the latest state. */
function useCommentMutation<T>(mutationFn: (input: T) => Promise<Comment>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSettled: () => queryClient.invalidateQueries({ queryKey: commentsKey }),
  })
}

export function useCreateComment() {
  return useCommentMutation(({ about, body }: { about: CommentAbout; body: string }) =>
    createComment(about, body),
  )
}

export function useUpdateComment() {
  return useCommentMutation(({ comment, body }: { comment: Comment; body: string }) =>
    updateComment(comment, body),
  )
}

export function useDeleteComment() {
  return useCommentMutation(deleteComment)
}

const lastViewedKey = (projectId: string) => `lastViewed[${projectId}]`

/**
 * Records that the Project is being viewed now, returning when it was last
 * viewed in this browser, or null if never (or storage is unavailable).
 */
export function markViewed(projectId: string, now = new Date()): string | null {
  let previous: string | null = null
  try {
    previous = localStorage.getItem(lastViewedKey(projectId))
  } catch {
    // Unreadable storage counts as never viewed.
  }
  try {
    localStorage.setItem(lastViewedKey(projectId), now.toISOString())
  } catch {
    // Nothing to do: the next visit just won't highlight anything.
  }
  return previous
}

/**
 * Whether a Comment arrived since the Project was last viewed, from someone
 * other than the Acting Person. Without a previous view nothing is new.
 */
export function isNew(comment: Comment, since: string | null, actingId: string | null): boolean {
  return (
    since !== null &&
    !comment.deleted &&
    comment.author.id !== actingId &&
    Date.parse(comment.created_at) > Date.parse(since)
  )
}

/** What the Project page shares with every Comments disclosure on it. */
export interface CommentsContext {
  projectId: Ref<string>
  comments: Ref<Comment[]>
  /** When the Project was last viewed before this page visit. */
  since: Ref<string | null>
  actingId: Ref<string | null>
  /** The Acting Person is a Member and the Project isn't archived. */
  canComment: Ref<boolean>
}

const contextKey: InjectionKey<CommentsContext> = Symbol('comments')

export function provideComments(context: CommentsContext) {
  provide(contextKey, context)
}

/** The Comments about one target, and whether each is new, from the Project page. */
export function useTargetComments(targetType: CommentTarget, targetId: Ref<string>) {
  const context = inject(contextKey)
  if (!context) throw new Error('useTargetComments needs provideComments on the Project page')
  const comments = computed(() =>
    context.comments.value
      .filter((c) => c.target_type === targetType && c.target_id === targetId.value)
      .map((comment) => ({
        comment,
        isNew: isNew(comment, context.since.value, context.actingId.value),
      })),
  )
  return { ...context, comments }
}
