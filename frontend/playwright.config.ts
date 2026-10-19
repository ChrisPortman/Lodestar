// Browser smoke tests against the real release binary, which embeds
// frontend/dist (#34). Run with `just e2e`, which builds both first.
import { mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { defineConfig, devices } from '@playwright/test'

const port = Number(process.env.E2E_PORT ?? 18080)
// A fresh database and session key per run, so tests never see stale data.
const dataDir = mkdtempSync(join(tmpdir(), 'lodestar-e2e-'))

export default defineConfig({
  testDir: 'e2e',
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: 'list',
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: '../target/release/lodestar',
    url: `http://127.0.0.1:${port}/`,
    reuseExistingServer: false,
    env: {
      LODESTAR_SERVER__PORT: String(port),
      LODESTAR_DATABASE__URL: `sqlite://${join(dataDir, 'lodestar.db')}`,
      LODESTAR_SESSION__KEY_FILE: join(dataDir, 'session.key'),
    },
  },
})
