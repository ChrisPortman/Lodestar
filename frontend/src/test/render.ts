import { render } from '@testing-library/vue'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createMemoryHistory } from 'vue-router'
import App from '../App.vue'
import { createAppRouter } from '../router'
import { server, fakeBackend, type Person } from './server'

/** Renders the whole app at `path` against an in-memory backend. */
export async function renderApp(path: string, people: Person[], actingId: string | null = null) {
  const backend = fakeBackend(people, actingId)
  server.use(...backend.handlers)
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const router = createAppRouter(createMemoryHistory(), queryClient)
  router.push(path)
  await router.isReady()
  const view = render(App, { global: { plugins: [router, [VueQueryPlugin, { queryClient }]] } })
  return { ...view, router, backend }
}
