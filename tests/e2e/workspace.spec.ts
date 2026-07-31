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
  // The trigger `<button>` (workspace_switcher.rs) carries a fixed
  // `aria-label="workspace.switch_label"` ("Switch workspace") that overrides
  // the visible workspace-name text for accessible-name purposes — the name
  // is NOT "Roommates". Rows inside the panel are `<a role="menuitem">`,
  // whose explicit role overrides the implicit `link` role, so they must be
  // queried as `menuitem`, not `link`.
  test('opens dropdown and lists all workspaces', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1');
    await authedPage.getByRole('button', { name: 'Switch workspace' }).click();
    await expect(authedPage.getByRole('menuitem', { name: /Personal/ })).toBeVisible();
    await expect(authedPage.getByRole('menuitem', { name: /Old Group/ })).toBeVisible();
  });

  test('switches workspace via dropdown', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1');
    await authedPage.getByRole('button', { name: 'Switch workspace' }).click();
    await authedPage.getByRole('menuitem', { name: /Personal/ }).click();
    await authedPage.waitForURL(/\/w\/test-dm1/);
  });
});
