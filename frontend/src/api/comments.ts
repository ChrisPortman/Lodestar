import { client, ifMatch, unwrap } from './client'
import type { components } from './schema'

export type Comment = components['schemas']['Comment']
export type CommentTarget = components['schemas']['TargetTypeParam']

const PAGE = 1000

/** Every Comment in a Project, on it or its items, deleted ones included, oldest first. */
export async function listComments(projectId: string): Promise<Comment[]> {
  const comments: Comment[] = []
  for (let offset = 0; ; offset += PAGE) {
    const page = unwrap(
      await client.GET('/api/v1/comments', {
        params: { query: { project_id: projectId, limit: PAGE, offset } },
      }),
    )
    comments.push(...page.items)
    if (page.items.length === 0 || comments.length >= page.total) return comments
  }
}

export interface CommentAbout {
  projectId: string
  targetType: CommentTarget
  targetId: string
}

export async function createComment(about: CommentAbout, body: string): Promise<Comment> {
  return unwrap(
    await client.POST('/api/v1/comments', {
      body: {
        project_id: about.projectId,
        target_type: about.targetType,
        target_id: about.targetId,
        body,
      } as Comment,
    }),
  )
}

export async function updateComment(comment: Comment, body: string): Promise<Comment> {
  return unwrap(
    await client.PUT('/api/v1/comments/{id}', {
      params: { path: { id: comment.id }, header: { 'if-match': ifMatch(comment.version) } },
      body: { ...comment, body },
    }),
  )
}

export async function deleteComment(comment: Comment): Promise<Comment> {
  return unwrap(
    await client.PATCH('/api/v1/comments/{id}', {
      params: { path: { id: comment.id }, header: { 'if-match': ifMatch(comment.version) } },
      headers: { 'Content-Type': 'application/merge-patch+json' },
      body: { deleted: true },
    }),
  )
}
