import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  server: {
    proxy: {
      '/api': process.env.LODESTAR_BACKEND ?? 'http://127.0.0.1:8080',
    },
  },
  test: {
    environment: 'jsdom',
    // A zone away from UTC, so tests notice times shown in UTC by mistake.
    env: { TZ: 'Australia/Sydney' },
    setupFiles: ['src/test/setup.ts'],
    include: ['src/**/*.spec.ts'],
  },
})
