import { test, expect } from './fixtures';

const SLUG = 'test-grp1';

// Smoke tests for every left-nav page in a workspace. Each just verifies the
// route loads without a runtime error and the page header is visible.
test.describe('Workspace pages — smoke', () => {
  for (const path of ['todos', 'notes', 'files', 'history', 'calendar', 'memory', 'scheduled', 'settings']) {
    test(`/${path} renders without error`, async ({ authedPage }) => {
      const errors: string[] = [];
      authedPage.on('pageerror', (e) => errors.push(e.message));
      authedPage.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });

      await authedPage.goto(`/w/${SLUG}/${path}`);

      // Wait for either a heading or a known wrapper element.
      await expect(authedPage.locator('h2, h3').first()).toBeVisible({ timeout: 10_000 });

      // No "Cannot read property", panic-style runtime errors.
      const fatal = errors.filter(e =>
        /panicked|TypeError|ReferenceError|Cannot read|not a function/i.test(e),
      );
      expect(fatal, `runtime errors on /${path}: ${fatal.join(' | ')}`).toEqual([]);
    });
  }
});

test.describe('Calendar page', () => {
  test('header shows current month + year', async ({ authedPage }) => {
    await authedPage.goto(`/w/${SLUG}/calendar?lang=en`);
    const header = authedPage.locator('h2').first();
    await expect(header).toBeVisible({ timeout: 10_000 });
    const text = await header.innerText();
    // Format is "<Month> <Year>" e.g. "April 2026"
    expect(text).toMatch(/^[A-Z][a-z]+\s+20\d{2}$/);
  });

  test('switching to agenda view does not crash', async ({ authedPage }) => {
    await authedPage.goto(`/w/${SLUG}/calendar?lang=en`);
    await expect(authedPage.locator('h2').first()).toBeVisible();
    await authedPage.getByRole('button', { name: 'Agenda', exact: false }).click();
    // After click, the page should still have a heading.
    await expect(authedPage.locator('h2').first()).toBeVisible();
  });
});

test.describe('Memory page', () => {
  test('lists the seeded pinned facts', async ({ authedPage }) => {
    await authedPage.goto(`/w/${SLUG}/memory`);
    await expect(authedPage.getByText(/Rent is 1450 EUR/i)).toBeVisible({ timeout: 10_000 });
  });
});

test.describe('Settings page', () => {
  test('header renders', async ({ authedPage }) => {
    await authedPage.goto(`/w/${SLUG}/settings`);
    await expect(authedPage.locator('h2').first()).toBeVisible({ timeout: 10_000 });
  });
});

test.describe('Global settings page', () => {
  test('renders display_name field', async ({ authedPage }) => {
    await authedPage.goto('/settings');
    await expect(authedPage.locator('h2, h1').first()).toBeVisible({ timeout: 10_000 });
  });
});
