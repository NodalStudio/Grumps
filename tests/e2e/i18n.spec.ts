import { test, expect } from './fixtures';

test.describe('i18n — day initials in "This week" calendar', () => {
  test('renders English initials when locale=en', async ({ authedPage }) => {
    // Force the locale via query string (the SPA reads ?lang= first).
    await authedPage.goto('/w/test-grp1?lang=en');

    // Wait for the calendar widget to render. The 7 day-name pills are
    // direct children of a 7-col grid inside the "This week" widget.
    const grid = authedPage.locator('div.grid.grid-cols-7').first();
    await expect(grid).toBeVisible({ timeout: 10_000 });

    // Each cell starts with the day initial as its first child div.
    const initials: string[] = [];
    const cellCount = await grid.locator(':scope > div').count();
    for (let i = 0; i < Math.min(cellCount, 7); i++) {
      const cell = grid.locator(':scope > div').nth(i);
      const initial = await cell.locator(':scope > div').first().innerText();
      initials.push(initial.trim());
    }
    expect(initials).toEqual(['M', 'T', 'W', 'T', 'F', 'S', 'S']);
  });

  test('renders French initials when locale=fr', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1?lang=fr');
    const grid = authedPage.locator('div.grid.grid-cols-7').first();
    await expect(grid).toBeVisible({ timeout: 10_000 });

    const initials: string[] = [];
    const cellCount = await grid.locator(':scope > div').count();
    for (let i = 0; i < Math.min(cellCount, 7); i++) {
      const cell = grid.locator(':scope > div').nth(i);
      const initial = await cell.locator(':scope > div').first().innerText();
      initials.push(initial.trim());
    }
    expect(initials).toEqual(['L', 'M', 'M', 'J', 'V', 'S', 'D']);
  });
});

test.describe('i18n — RTL + html attrs', () => {
  test('Arabic locale sets <html dir="rtl" lang="ar">', async ({ authedPage }) => {
    await authedPage.goto('/dashboard?lang=ar');
    // Wait for AuthGate to settle.
    await expect(authedPage.locator('h2, h1').first()).toBeVisible({ timeout: 10_000 });
    const dir = await authedPage.evaluate(() => document.documentElement.getAttribute('dir'));
    const lang = await authedPage.evaluate(() => document.documentElement.getAttribute('lang'));
    expect(dir).toBe('rtl');
    expect(lang).toBe('ar');
  });

  test('English locale sets <html dir="ltr" lang="en">', async ({ authedPage }) => {
    await authedPage.goto('/dashboard?lang=en');
    await expect(authedPage.locator('h2, h1').first()).toBeVisible({ timeout: 10_000 });
    const dir = await authedPage.evaluate(() => document.documentElement.getAttribute('dir'));
    const lang = await authedPage.evaluate(() => document.documentElement.getAttribute('lang'));
    expect(dir).toBe('ltr');
    expect(lang).toBe('en');
  });

  test('locale persists across reload via localStorage', async ({ authedPage }) => {
    await authedPage.goto('/dashboard?lang=de');
    await expect(authedPage.locator('h2, h1').first()).toBeVisible({ timeout: 10_000 });
    // Now reload without ?lang=. The SPA should pick up the previous one
    // from localStorage.
    await authedPage.goto('/dashboard');
    await expect(authedPage.locator('h2, h1').first()).toBeVisible();
    const lang = await authedPage.evaluate(() => document.documentElement.getAttribute('lang'));
    expect(lang).toBe('de');
  });
});
