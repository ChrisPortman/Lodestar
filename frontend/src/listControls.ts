// Sort and filter for the meeting view panels (FR-92, FR-23). Panels hold
// every item of the Project, so this works on the loaded list.
import { reactive } from 'vue'

export type StateFilter = 'all' | 'open' | 'closed'

export interface ListQuery {
  sort: string
  /** '' for anyone, 'unassigned', or a Person ID. */
  assignee: string
  state: StateFilter
  /** `YYYY-MM-DD`, or '' for no limit. */
  dueBy: string
}

/** How to read the fields a panel can sort and filter by. */
export interface Fields<T> {
  reference: (item: T) => string
  assignee?: (item: T) => { id: string; display_name: string } | null
  due?: (item: T) => string | null
  score?: (item: T) => number
}

export function useListQuery(): ListQuery {
  return reactive({ sort: 'reference', assignee: '', state: 'all', dueBy: '' })
}

const byReference = (a: string, b: string) => a.localeCompare(b, undefined, { numeric: true })

/** Missing values sort last. */
function nullsLast<V>(a: V | null, b: V | null, cmp: (a: V, b: V) => number) {
  if (a === null) return b === null ? 0 : 1
  return b === null ? -1 : cmp(a, b)
}

/** The items that pass the query's filters, in its order. */
export function applyQuery<T>(items: T[], query: ListQuery, fields: Fields<T>): T[] {
  const { assignee, due, score } = fields
  const filtered = items.filter((item) => {
    if (assignee && query.assignee) {
      const who = assignee(item)
      if (query.assignee === 'unassigned' ? who !== null : who?.id !== query.assignee) return false
    }
    if (due && query.dueBy) {
      const date = due(item)
      if (date === null || date > query.dueBy) return false
    }
    return true
  })
  const ref = (a: T, b: T) => byReference(fields.reference(a), fields.reference(b))
  const order: Record<string, (a: T, b: T) => number> = {
    reference: ref,
    due: (a, b) => nullsLast(due?.(a) ?? null, due?.(b) ?? null, (x, y) => x.localeCompare(y)),
    assignee: (a, b) =>
      nullsLast(assignee?.(a)?.display_name ?? null, assignee?.(b)?.display_name ?? null, (x, y) =>
        x.localeCompare(y),
      ),
    score: (a, b) => (score?.(b) ?? 0) - (score?.(a) ?? 0),
  }
  const cmp = order[query.sort] ?? ref
  return filtered.sort((a, b) => cmp(a, b) || ref(a, b))
}
