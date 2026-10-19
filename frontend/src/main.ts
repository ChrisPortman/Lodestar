import { createApp } from 'vue'
import { createWebHistory } from 'vue-router'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import './style.css'
import App from './App.vue'
import { createAppRouter } from './router'

const queryClient = new QueryClient()
const router = createAppRouter(createWebHistory(), queryClient)

createApp(App).use(router).use(VueQueryPlugin, { queryClient }).mount('#app')
