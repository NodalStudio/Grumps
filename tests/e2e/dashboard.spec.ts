import { test, expect } from './fixtures';

test.describe('Dashboard', () => {
  test('shows 3 seeded workspaces with correct shapes', async ({ authedPage }) => {
    await authedPage.goto('/dashboard');
    await expect(authedPage.getByRole('heading', { name: 'Roommates' })).toBeVisible();
    await expect(authedPage.getByRole('heading', { name: 'Personal' })).toBeVisible();
    await expect(authedPage.getByRole('heading', { name: 'Old Group' })).toBeVisible();
    await expect(authedPage.getByText('TELEGRAM GROUP').first()).toBeVisible();
    await expect(authedPage.getByText(/TELEGRAM DM · JUST YOU/i)).toBeVisible();
  });

  test('archived workspace has visible chip and reduced opacity', async ({ authedPage }) => {
    await authedPage.goto('/dashboard?lang=en');
    // The "Archived" chip is uppercased via CSS but the DOM text is mixed-case.
    // Force locale=en so the chip text is deterministic across machines.
    await expect(authedPage.getByText('Archived', { exact: true })).toBeVisible();
    const oldGroupCard = authedPage.locator('a').filter({ hasText: 'Old Group' });
    // Reduced opacity via Tailwind opacity-60
    const opacity = await oldGroupCard.evaluate((el) => getComputedStyle(el).opacity);
    expect(parseFloat(opacity)).toBeLessThan(0.8);
  });

  test('clicking a workspace card navigates to /w/<slug>', async ({ authedPage }) => {
    await authedPage.goto('/dashboard');
    await authedPage.getByRole('heading', { name: 'Roommates' }).click();
    await authedPage.waitForURL(/\/w\/test-grp1/);
  });

  test('"Add Grumps to another group" links to t.me deep link', async ({ authedPage }) => {
    await authedPage.goto('/dashboard');
    const addLink = authedPage.getByRole('link', { name: /add grumps to another group/i });
    await expect(addLink).toHaveAttribute('href', /t\.me\/HeyGrumpsBot\?startgroup=true/);
  });

  test('Account button navigates to /settings', async ({ authedPage }) => {
    await authedPage.goto('/dashboard');
    await authedPage.getByRole('link', { name: 'Account' }).click();
    await authedPage.waitForURL(/\/settings$/);
  });
});
