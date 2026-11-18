import { test, expect } from '@playwright/test';
import { E2E_API } from './api-base';

const TEST_EMAIL = 'e2e-projects@test.local';
const TEST_PASSWORD = 'Test1234!';

test.describe('Projects CRUD', () => {
  test.beforeAll(async ({ request }) => {
    // Ensure test user exists
    const res = await request.post(`${E2E_API}/api/v1/auth/register`, {
      data: { email: TEST_EMAIL, password: TEST_PASSWORD },
    });
    if (res.status() !== 201 && res.status() !== 409) {
      throw new Error(`Failed to create test user: ${res.status()}`);
    }
  });

  test.beforeEach(async ({ page }) => {
    // Login before each test
    await page.goto('/login');
    await page.getByLabel('Email').fill(TEST_EMAIL);
    await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
    await page.getByRole('button', { name: 'Sign In' }).click();
    await page.waitForURL((url) => url.pathname !== '/login', { timeout: 10000 });
    await page.goto('/projects');
  });

  test('shows My Projects heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'My Projects' })).toBeVisible();
  });

  test('shows Create Project button', async ({ page }) => {
    // Wait for page to load
    await expect(page.getByRole('heading', { name: 'My Projects' })).toBeVisible();

    // Either empty state or create button should be visible
    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Wait for either CTA instead of racing the empty-state render.
    await expect(emptyStateButton.or(createButton)).toBeVisible();
  });

  test('creates a project via Create Project button', async ({ page }) => {
    const projectName = `Test Project ${Date.now()}`;

    // Click the create button (either empty state or header button)
    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Either CTA may render depending on whether the list is empty —
    // auto-wait for whichever is present (empty state needs a beat to settle).
    await expect(emptyStateButton.or(createButton)).toBeVisible();
    if (await emptyStateButton.isVisible()) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    const input = page.getByLabel('Project name');
    await expect(input).toBeVisible();
    await input.fill(projectName);
    await page.getByRole('button', { name: 'Save' }).click();

    // Project should appear in list (use more specific locator)
    await expect(page.getByRole('main').getByText(projectName)).toBeVisible();
  });

  test('cancels project creation with Escape key', async ({ page }) => {
    // Click create button
    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Either CTA may render depending on whether the list is empty —
    // auto-wait for whichever is present (empty state needs a beat to settle).
    await expect(emptyStateButton.or(createButton)).toBeVisible();
    if (await emptyStateButton.isVisible()) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    const input = page.getByLabel('Project name');
    await expect(input).toBeVisible();

    await input.press('Escape');

    // Form should disappear
    await expect(input).not.toBeVisible();
  });

  test('cancels project creation with Cancel button', async ({ page }) => {
    // Click create button
    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Either CTA may render depending on whether the list is empty —
    // auto-wait for whichever is present (empty state needs a beat to settle).
    await expect(emptyStateButton.or(createButton)).toBeVisible();
    if (await emptyStateButton.isVisible()) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    const input = page.getByLabel('Project name');
    await expect(input).toBeVisible();

    await page.getByRole('button', { name: 'Cancel' }).click();

    // Form should disappear
    await expect(input).not.toBeVisible();
  });

  test('edits a project name', async ({ page }) => {
    // First create a project
    const originalName = `Edit Me ${Date.now()}`;
    const updatedName = `Updated ${Date.now()}`;

    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Either CTA may render depending on whether the list is empty —
    // auto-wait for whichever is present (empty state needs a beat to settle).
    await expect(emptyStateButton.or(createButton)).toBeVisible();
    if (await emptyStateButton.isVisible()) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    await page.getByLabel('Project name').fill(originalName);
    await page.getByRole('button', { name: 'Save' }).click();

    // Wait for project to appear
    await expect(page.getByRole('main').getByText(originalName)).toBeVisible();

    // Hover to reveal edit button
    const projectCard = page.getByRole('main').locator('div').filter({ hasText: originalName }).first();
    await projectCard.hover();

    // Click edit button
    await page.getByRole('button', { name: `Edit project ${originalName}` }).click();

    // Edit the name
    const editInput = page.getByLabel('Project name');
    await editInput.fill(updatedName);
    await editInput.press('Enter');

    // Updated name should appear
    await expect(page.getByRole('main').getByText(updatedName)).toBeVisible();
  });

  test('deletes a project with confirmation', async ({ page }) => {
    // First create a project
    const projectName = `Delete Me ${Date.now()}`;

    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Either CTA may render depending on whether the list is empty —
    // auto-wait for whichever is present (empty state needs a beat to settle).
    await expect(emptyStateButton.or(createButton)).toBeVisible();
    if (await emptyStateButton.isVisible()) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    await page.getByLabel('Project name').fill(projectName);
    await page.getByRole('button', { name: 'Save' }).click();

    // Wait for project to appear
    await expect(page.getByRole('main').getByText(projectName)).toBeVisible();

    // Hover to reveal delete button
    const projectCard = page.getByRole('main').locator('div').filter({ hasText: projectName }).first();
    await projectCard.hover();

    // Click delete button
    await page.getByRole('button', { name: `Delete project ${projectName}` }).click();

    // Confirmation dialog should appear
    await expect(page.getByRole('heading', { name: 'Delete project' })).toBeVisible();

    // Confirm deletion
    await page.getByRole('button', { name: 'Delete', exact: true }).click();

    // Project should be removed from main content
    await expect(page.getByRole('main').getByText(projectName)).not.toBeVisible();
  });

  test('cancels project deletion', async ({ page }) => {
    // First create a project
    const projectName = `Keep Me ${Date.now()}`;

    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Either CTA may render depending on whether the list is empty —
    // auto-wait for whichever is present (empty state needs a beat to settle).
    await expect(emptyStateButton.or(createButton)).toBeVisible();
    if (await emptyStateButton.isVisible()) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    await page.getByLabel('Project name').fill(projectName);
    await page.getByRole('button', { name: 'Save' }).click();

    // Wait for project to appear
    await expect(page.getByRole('main').getByText(projectName)).toBeVisible();

    // Hover to reveal delete button
    const projectCard = page.getByRole('main').locator('div').filter({ hasText: projectName }).first();
    await projectCard.hover();

    // Click delete button
    await page.getByRole('button', { name: `Delete project ${projectName}` }).click();

    // Cancel deletion
    await page.getByRole('button', { name: 'Cancel' }).click();

    // Project should still exist
    await expect(page.getByRole('main').getByText(projectName)).toBeVisible();
  });

  test('project card shows board count', async ({ page }) => {
    const projectName = `Board Count ${Date.now()}`;

    const emptyStateButton = page.getByRole('button', { name: 'Create your first project' });
    const createButton = page.getByRole('button', { name: 'Create Project' });

    // Either CTA may render depending on whether the list is empty —
    // auto-wait for whichever is present (empty state needs a beat to settle).
    await expect(emptyStateButton.or(createButton)).toBeVisible();
    if (await emptyStateButton.isVisible()) {
      await emptyStateButton.click();
    } else {
      await createButton.click();
    }

    await page.getByLabel('Project name').fill(projectName);
    await page.getByRole('button', { name: 'Save' }).click();

    // Should show "0 boards" in main content (use first to avoid sidebar match)
    await expect(page.getByRole('main').getByText('0 boards').first()).toBeVisible();
  });
});
