import { test, expect } from './fixtures';

test.describe('Workspace overview', () => {
  test('header displays the workspace name (not slug)', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1');
    // Wait for AuthGate to provide session.
    await expect(authedPage.locator('h2').first()).toHaveText('Roommates', { timeout: 10_000 });
  });

  test('overview stats reflect seeded D1 data', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1');
    // Seeded: 5 open todos, 2 notes, 0 done this week, 0 files.
    await expect(authedPage.getByText('5').first()).toBeVisible({ timeout: 10_000 });
    await expect(authedPage.getByText('OPEN TODOS')).toBeVisible();
    await expect(authedPage.getByText('NOTES').first()).toBeVisible();
  });

  test('sidebar footer shows display_name + role from session', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1');
    await expect(authedPage.getByText('Tester').first()).toBeVisible();
    await expect(authedPage.getByText('ADMIN').first()).toBeVisible();
  });
});

test.describe('Workspace switcher', () => {
  test('opens dropdown and lists all workspaces', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1');
    await authedPage.getByRole('button', { name: /Roommates/i }).click();
    await expect(authedPage.getByRole('link', { name: /Personal/ })).toBeVisible();
    await expect(authedPage.getByRole('link', { name: /Old Group/ })).toBeVisible();
  });

  test('switches workspace via dropdown', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1');
    await authedPage.getByRole('button', { name: /Roommates/i }).click();
    await authedPage.getByRole('link', { name: /Personal/ }).click();
    await authedPage.waitForURL(/\/w\/test-dm1/);
  });
});
