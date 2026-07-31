import { test, expect } from './fixtures';

test.describe('Dashboard', () => {
  test('shows 3 seeded workspaces with correct shapes', async ({ authedPage }) => {
    await authedPage.goto('/dashboard');
    await expect(authedPage.getByRole('heading', { name: 'Roommates' })).toBeVisible();
    await expect(authedPage.getByRole('heading', { name: 'Personal' })).toBeVisible();
    await expect(authedPage.getByRole('heading', { name: 'Old Group' })).toBeVisible();
    // `format_shape` (dashboard.rs) renders "Telegram Group" / "Telegram DM" —
    // no "· Just you" suffix.
    await expect(authedPage.getByText('Telegram Group').first()).toBeVisible();
    await expect(authedPage.getByText('Telegram DM').first()).toBeVisible();
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

  test('"Add Grumps to another group" opens the help modal with a t.me link', async ({ authedPage }) => {
    // With workspaces already seeded, dashboard.rs renders the non-empty
    // `Grid`, where "+ Add Grumps to another group" is a `<button>` that
    // opens an `AddGroupHelp` dialog — not a direct `<a href>` (that shape
    // only exists in the empty-state CTA, which isn't rendered here).
    await authedPage.goto('/dashboard');
    await authedPage.getByRole('button', { name: /add grumps to another group/i }).click();
    const openLink = authedPage.getByRole('link', { name: /open @HeyGrumpsBot/i });
    await expect(openLink).toHaveAttribute('href', 'https://t.me/HeyGrumpsBot');
  });

  test('Account button opens the account drawer', async ({ authedPage }) => {
    // Account settings live in a global slide-over drawer (account_drawer.rs),
    // not a separate /settings page.
    await authedPage.goto('/dashboard');
    await authedPage.getByRole('button', { name: 'Account' }).click();
    // `exact: true` — a substring match would also hit the drawer's own
    // "Linked accounts" sub-heading.
    await expect(authedPage.getByRole('heading', { name: 'Account', exact: true })).toBeVisible();
    await expect(authedPage.getByText(/linked accounts/i)).toBeVisible();
  });
});
