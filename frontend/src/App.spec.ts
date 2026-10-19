import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import App from './App.vue'

describe('App', () => {
  it('shows the product name', () => {
    expect(mount(App).text()).toContain('Lodestar')
  })
})
