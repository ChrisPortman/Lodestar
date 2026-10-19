import { afterAll, afterEach, beforeAll } from 'vitest'
import { cleanup } from '@testing-library/vue'
import { server } from './server'

beforeAll(() => server.listen({ onUnhandledFrame: 'error' }))
afterEach(() => {
  cleanup()
  server.resetHandlers()
})
afterAll(() => server.close())
