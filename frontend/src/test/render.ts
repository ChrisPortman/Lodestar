import { render } from '@testing-library/vue'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createMemoryHistory } from 'vue-router'
import App from '../App.vue'
import { createAppRouter } from '../router'
import {
  server,
  fakeBackend,
  type Person,
  type Project,
  type Risk,
  type Action,
  type Blocker,
  type Comment,
  type MeetingLock,
} from './server'

/** Renders the whole app at `path` against an in-memory backend. */
export async function renderApp(
  path: string,
  people: Person[],
  actingId: string | null = null,
  projects: Project[] = [],
  risks: Risk[] = [],
  actions: Action[] = [],
  blockers: Blocker[] = [],
  comments: Comment[] = [],
  lock: MeetingLock | null = null,
) {
  const backend = fakeBackend(people, actingId, projects, risks, actions, blockers, comments, lock)
  server.use(...backend.handlers)
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const router = createAppRouter(createMemoryHistory(), queryClient)
  router.push(path)
  await router.isReady()
  const view = render(App, { global: { plugins: [router, [VueQueryPlugin, { queryClient }]] } })
  return { ...view, router, backend }
}
