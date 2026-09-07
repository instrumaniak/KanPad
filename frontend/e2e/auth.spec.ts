import { test, expect } from '@playwright/test';

const TEST_PASSWORD = 'Test1234!';
// Fixed email — cleaned up before run via DB
const TEST_EMAIL = 'e2e-login@test.local';

test.describe('Auth flows', () => {
  test.describe('Registration', () => {
    test('renders registration form with all fields', async ({ page }) => {
      await page.goto('/register');

      await expect(page.getByRole('heading', { name: 'KanPad' })).toBeVisible();
      await expect(page.getByLabel('Email')).toBeVisible();
      await expect(page.getByLabel('Password', { exact: true })).toBeVisible();
      await expect(page.getByLabel('Confirm Password')).toBeVisible();
      await expect(page.getByRole('button', { name: 'Register' })).toBeVisible();
    });

    test('shows validation errors for empty fields', async ({ page }) => {
      await page.goto('/register');

      await page.getByRole('button', { name: 'Register' }).click();

      await expect(page.getByText('Email is required')).toBeVisible();
      await expect(page.getByText('Password is required')).toBeVisible();
      await expect(page.getByText('Please confirm your password')).toBeVisible();
    });

    test('shows validation error for weak password', async ({ page }) => {
      await page.goto('/register');

      await page.getByLabel('Email').fill('test@example.com');
      await page.getByLabel('Password', { exact: true }).fill('short');
      await page.getByRole('button', { name: 'Register' }).click();

      await expect(page.getByText('Password must be at least 8 characters')).toBeVisible();
    });

    test('shows validation error for mismatched passwords', async ({ page }) => {
      await page.goto('/register');

      await page.getByLabel('Email').fill('test@example.com');
      await page.getByLabel('Password', { exact: true }).fill('Test1234!');
      await page.getByLabel('Confirm Password').fill('Different123!');
      await page.getByRole('button', { name: 'Register' }).click();

      await expect(page.getByText('Passwords do not match')).toBeVisible();
    });

    test('registers a new user and redirects to app', async ({ page }) => {
      const uniqueEmail = `register-${Date.now()}@e2e.test`;
      await page.goto('/register');

      await page.getByLabel('Email').fill(uniqueEmail);
      await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
      await page.getByLabel('Confirm Password').fill(TEST_PASSWORD);
      await page.getByRole('button', { name: 'Register' }).click();

      await page.waitForURL((url) => url.pathname === '/', { timeout: 15000 });
      await expect(page.getByRole('heading', { name: 'My Boards' })).toBeVisible();
    });
  });

  test.describe('Login', () => {
    test.beforeAll(async ({ request }) => {
      // Ensure test user exists via API
      const res = await request.post('http://localhost:3000/api/auth/register', {
        data: { email: TEST_EMAIL, password: TEST_PASSWORD },
      });
      // 201 = created, 409 = already exists — both OK
      if (res.status() !== 201 && res.status() !== 409) {
        throw new Error(`Failed to create test user: ${res.status()}`);
      }
    });

    test('renders login form with all fields', async ({ page }) => {
      await page.goto('/login');

      await expect(page.getByRole('heading', { name: 'KanPad' })).toBeVisible();
      await expect(page.getByLabel('Email')).toBeVisible();
      await expect(page.getByLabel('Password', { exact: true })).toBeVisible();
      await expect(page.getByRole('button', { name: 'Sign In' })).toBeVisible();
    });

    test('shows validation errors for empty fields', async ({ page }) => {
      await page.goto('/login');

      await page.getByRole('button', { name: 'Sign In' }).click();

      await expect(page.getByText('Email is required')).toBeVisible();
      await expect(page.getByText('Password is required')).toBeVisible();
    });

    test('shows error for wrong credentials', async ({ page }) => {
      await page.goto('/login');

      await page.getByLabel('Email').fill('nonexistent@example.com');
      await page.getByLabel('Password', { exact: true }).fill('WrongPass123!');
      await page.getByRole('button', { name: 'Sign In' }).click();

      // Should stay on /login
      await expect(page).toHaveURL('/login');
    });

    test('logs in with valid credentials and redirects to app', async ({ page }) => {
      await page.goto('/login');

      await page.getByLabel('Email').fill(TEST_EMAIL);
      await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
      await page.getByRole('button', { name: 'Sign In' }).click();

      await page.waitForURL((url) => url.pathname !== '/login', { timeout: 10000 });
      await expect(page.getByText('KanPad')).toBeVisible();
    });

    test('has link to register page', async ({ page }) => {
      await page.goto('/login');

      await page.getByRole('link', { name: 'Register' }).click();

      await expect(page).toHaveURL('/register');
    });
  });

  test.describe('Logout', () => {
    test('logs out and redirects to login', async ({ page }) => {
      // Login first
      await page.goto('/login');
      await page.getByLabel('Email').fill(TEST_EMAIL);
      await page.getByLabel('Password', { exact: true }).fill(TEST_PASSWORD);
      await page.getByRole('button', { name: 'Sign In' }).click();

      await page.waitForURL((url) => url.pathname !== '/login', { timeout: 10000 });

      // Logout
      await page.getByRole('button', { name: TEST_EMAIL }).click();
      await page.getByRole('menuitem', { name: 'Log out' }).click();

      await expect(page).toHaveURL('/login');
    });
  });

  test.describe('Auth guard', () => {
    test('redirects unauthenticated user from / to /login', async ({ page }) => {
      await page.goto('/');

      await expect(page).toHaveURL('/login');
    });
  });
});
