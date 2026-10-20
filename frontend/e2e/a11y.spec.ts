import { expect, test } from '@playwright/test'
import {
  actAs,
  seedAction,
  seedBlocker,
  seedProject,
  seedReportItems,
  seedRisk,
  setHealthRating,
  updateAction,
} from './seed'
import { THEMES, expectAccessible, useTheme } from './axe'

for (const theme of THEMES) {
  test.describe(`${theme} theme`, () => {
    test.beforeEach(async ({ page }) => useTheme(page, theme))

    test('the Acting Person picker is accessible', async ({ page }) => {
      await page.goto('/')
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
      await expectAccessible(page)
    })

    test('the Projects list is accessible', async ({ page, request }) => {
      const { person } = await seedProject(request, `Listed ${theme}`)
      await actAs(page, person)
      await page.goto('/projects')
      await expect(page.getByRole('link', { name: `Listed ${theme}` })).toBeVisible()
      await expectAccessible(page)
    })

    test('the New Project dialog is accessible', async ({ page, request }) => {
      const { person } = await seedProject(request, `Dialog ${theme}`)
      await actAs(page, person)
      await page.goto('/projects')
      await page.getByRole('button', { name: 'New Project' }).click()
      await expect(page.getByRole('dialog', { name: 'New Project' })).toBeVisible()
      await expectAccessible(page)
    })

    test('a Project page is accessible', async ({ page, request }) => {
      const { person, project } = await seedProject(request, `Project ${theme}`)
      await seedAction(request, project.id, 'Book the venue')
      const overdue = await seedAction(request, project.id, 'Send invites')
      await updateAction(request, overdue, { assignee: person, due_date: '2001-01-01' })
      const blocked = await seedAction(request, project.id, 'Print badges')
      await seedBlocker(request, project.id, 'Printer broken', [blocked.id])
      await seedRisk(request, project.id, 'Venue cancels', 5, 4)
      await seedRisk(request, project.id, 'Rain', 3, 2)
      await seedRisk(request, project.id, 'Late speaker', 1, 2)
      await setHealthRating(request, project.id, 'amber', 'Venue pending')
      await actAs(page, person)
      await page.goto(`/projects/${project.id}`)
      await expect(page.getByRole('region', { name: 'Actions' })).toContainText('Blocked by')
      await expect(page.getByRole('region', { name: 'Blockers' })).toContainText('Printer broken')
      await expect(page.getByRole('region', { name: 'Risks' })).toContainText('Late speaker')
      await expect(page.getByRole('status', { name: 'Health Rating' })).toHaveText('▲ Amber')
      await expectAccessible(page)
      // Hover and keyboard focus states keep their contrast too.
      await page.getByRole('button', { name: 'Members' }).hover()
      await expectAccessible(page)
      await page.getByRole('button', { name: 'Members' }).focus()
      await expectAccessible(page)
    })

    for (const rating of ['red', 'green'] as const) {
      test(`a ${rating} Health Rating chip is accessible`, async ({ page, request }) => {
        const { person, project } = await seedProject(request, `Rated ${rating} ${theme}`)
        await setHealthRating(request, project.id, rating)
        await actAs(page, person)
        await page.goto(`/projects/${project.id}`)
        await expect(page.getByRole('status', { name: 'Health Rating' })).toHaveText(
          rating === 'red' ? '✕ Red' : '✓ Green',
        )
        await expectAccessible(page)
      })
    }

    test('Project on a Page is accessible', async ({ page, request }) => {
      const { person, project } = await seedProject(request, `Report ${theme}`)
      await seedAction(request, project.id, 'Print the agenda')
      await seedReportItems(request, project)
      await actAs(page, person)
      await page.goto(`/projects/${project.id}/report`)
      await expect(page.getByRole('table', { name: /Actions/ })).toContainText('▲ Overdue')
      await expect(page.getByText('▲ Red')).toBeVisible()
      await expect(page.getByText('▲ High 20')).toBeVisible()
      await expectAccessible(page)
    })

    test('the People page is accessible', async ({ page, request }) => {
      const { person } = await seedProject(request, `People ${theme}`)
      await actAs(page, person)
      await page.goto('/people')
      await expect(page.getByRole('list', { name: 'People' })).toBeVisible()
      await expectAccessible(page)
    })
  })
}

test('a chosen theme is remembered across reloads, over the OS preference', async ({ page }) => {
  await useTheme(page, 'light')
  await page.goto('/')
  await page.getByRole('button', { name: 'Dark mode' }).click()
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect(page.getByRole('button', { name: 'Dark mode' })).toHaveAttribute(
    'aria-pressed',
    'true',
  )
})

test('printing uses the light palette', async ({ page }) => {
  await useTheme(page, 'dark')
  await page.goto('/')
  await page.emulateMedia({ media: 'print' })
  const background = await page.evaluate(() => getComputedStyle(document.body).backgroundColor)
  expect(background).toBe('rgb(245, 247, 250)')
})
