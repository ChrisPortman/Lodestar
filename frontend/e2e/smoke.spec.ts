import { expect, test } from '@playwright/test'
import { actAs, seedAction, seedPerson, seedProject, updateAction } from './seed'

test('pick a person, create a Project, add an Action and change its Health Rating', async ({
  page,
  request,
}) => {
  const person = await seedPerson(request, `Picker ${Date.now()}`)

  await page.goto('/')
  await page.getByRole('button', { name: person.display_name }).click()
  await expect(page).toHaveURL(/\/projects$/)

  await page.getByRole('button', { name: 'New Project' }).click()
  const dialog = page.getByRole('dialog', { name: 'New Project' })
  await dialog.getByLabel('Name').fill('Smoke Test Project')
  await expect(dialog.getByLabel('Project Code')).not.toHaveValue('')
  await dialog.getByRole('button', { name: 'Create' }).click()
  await page.getByRole('link', { name: 'Smoke Test Project' }).click()
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Smoke Test Project')

  await page.getByRole('button', { name: 'Add Action' }).click()
  await page.getByLabel('Description', { exact: true }).fill('Book the venue')
  await page.getByRole('button', { name: 'Add', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Actions' })).toContainText('Book the venue')

  await page.getByRole('button', { name: 'Edit Health Rating' }).click()
  await page.getByRole('radio', { name: 'Amber' }).check()
  await page.getByLabel('Comment', { exact: true }).fill('Venue not yet confirmed')
  await page.getByRole('button', { name: 'Save' }).click()
  await expect(page.getByRole('status', { name: 'Health Rating' })).toHaveText('● Amber')
  await expect(page.getByText('Venue not yet confirmed')).toBeVisible()
})

test('the Project on a Page route renders the seeded Project', async ({ page, request }) => {
  const { person, project } = await seedProject(request, 'Report Project')
  await seedAction(request, project.id, 'Print the agenda')

  await actAs(page, person)
  await page.goto(`/projects/${project.id}/report`)
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Report Project')
  await expect(page.getByRole('table', { name: /Actions/ })).toContainText('Print the agenda')
})

test('saving an Action changed underneath shows the conflict notice', async ({ page, request }) => {
  const { person, project } = await seedProject(request, 'Conflict Project')
  const action = await seedAction(request, project.id, 'Original wording')

  await actAs(page, person)
  await page.goto(`/projects/${project.id}`)
  await page.getByRole('button', { name: `Edit ${action.reference}` }).click()
  await page.getByLabel(`Description of ${action.reference}`).fill('My wording')

  await updateAction(request, action, { description: 'Their wording' })
  await page.getByRole('button', { name: 'Save', exact: true }).click()

  await expect(page.getByRole('status').filter({ hasText: 'Changed elsewhere' })).toContainText(
    'Their wording',
  )
  await expect(page.getByLabel(`Description of ${action.reference}`)).toHaveValue('My wording')
})

test('run a Meeting and read its Minutes', async ({ page, request }) => {
  const { person, project } = await seedProject(request, 'Meeting Project')
  const action = await seedAction(request, project.id, 'Draft the agenda')

  await actAs(page, person)
  await page.goto(`/projects/${project.id}`)
  await page.getByRole('button', { name: 'Project menu' }).click()
  await page.getByRole('button', { name: 'Meetings' }).click()
  await page.getByRole('button', { name: 'Start a Meeting now' }).click()

  const panel = page.getByRole('dialog', { name: 'Meeting' })
  await panel.getByLabel('Meeting Comment', { exact: true }).fill('Agreed the venue')
  await panel.getByLabel('Meeting Comment', { exact: true }).press('Enter')
  await expect(panel.getByRole('list', { name: 'Meeting Comments' })).toContainText(
    'Agreed the venue',
  )
  await panel.getByRole('button', { name: 'Close' }).click()

  await page.getByRole('button', { name: `Edit ${action.reference}` }).click()
  await page.getByLabel(`Description of ${action.reference}`).fill('Send the agenda')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByRole('region', { name: 'Actions' })).toContainText('Send the agenda')

  await page.getByRole('button', { name: /● Meeting/ }).click()
  await panel.getByRole('button', { name: 'End Meeting…' }).click()
  await panel.getByRole('button', { name: 'Create Minutes' }).click()

  const reader = page.getByRole('dialog', { name: /Minutes/ })
  await expect(reader).toContainText('Description Draft the agenda → Send the agenda')
  await expect(reader).toContainText('Agreed the venue')
})
