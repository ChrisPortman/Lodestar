// Accessibility scans with axe-core (#54): WCAG 2.2 AA, in each theme.
import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'

export const THEMES = ['light', 'dark'] as const
export type Theme = (typeof THEMES)[number]

/**
 * Makes `page` open in `theme`, as the OS preference would, before any script
 * runs. Call it before the first `page.goto`.
 */
export async function useTheme(page: Page, theme: Theme) {
  await page.emulateMedia({ colorScheme: theme })
}

/** Fails the test with axe's violations if the page as it stands breaks WCAG 2.2 AA. */
export async function expectAccessible(page: Page) {
  await test.step('axe scan', async () => {
    // Let transitions settle, so colour contrast is measured on final colours.
    await page.waitForLoadState('networkidle')
    const results = await new AxeBuilder({ page })
      .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
      .analyze()
    const summary = results.violations.map((v) => ({
      id: v.id,
      help: v.help,
      nodes: v.nodes.map((n) => `${n.target.join(' ')}: ${n.failureSummary}`),
    }))
    expect(summary).toEqual([])
  })
}
