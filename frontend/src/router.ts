import { createRouter, type RouterHistory } from 'vue-router'
import type { QueryClient } from '@tanstack/vue-query'
import { fetchSession } from './session'
import HomeView from './views/HomeView.vue'
import PeopleView from './views/PeopleView.vue'
import ProjectsView from './views/ProjectsView.vue'
import PlaceholderView from './views/PlaceholderView.vue'

export function createAppRouter(history: RouterHistory, queryClient: QueryClient) {
  const router = createRouter({
    history,
    routes: [
      { path: '/', name: 'home', component: HomeView, meta: { public: true } },
      { path: '/projects', component: ProjectsView },
      { path: '/projects/:id', component: PlaceholderView, props: { title: 'Project' } },
      {
        path: '/projects/:id/report',
        component: PlaceholderView,
        props: { title: 'Project on a Page' },
      },
      { path: '/people', component: PeopleView },
    ],
  })

  // Without an Acting Person every page but the picker sends you to the picker.
  router.beforeEach(async (to) => {
    if (to.meta.public) return true
    const session = await fetchSession(queryClient)
    return session ? true : { name: 'home' }
  })

  return router
}
