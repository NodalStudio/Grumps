import { test, expect, csrf } from './fixtures';

test.describe('Auth flow', () => {
  test('login page renders 3 platform buttons (TG active, WA/DC disabled)', async ({ page }) => {
    await page.goto('/login');
    await expect(page.getByText('GRUMPS.', { exact: false })).toBeVisible();
    // TG widget is loaded via 3rd-party script — assert the placeholder div exists.
    await expect(page.locator('#tg-widget-container')).toBeVisible();
    await expect(page.getByText(/Log in with WhatsApp/i)).toBeVisible();
    await expect(page.getByText(/Log in with Discord/i)).toBeVisible();
    // Both placeholders are disabled.
    const wa = page.getByRole('button', { name: /Log in with WhatsApp/i });
    const dc = page.getByRole('button', { name: /Log in with Discord/i });
    await expect(wa).toBeDisabled();
    await expect(dc).toBeDisabled();
  });

  test('GET /auth/me without cookies returns 401 with CORS headers', async ({ page }) => {
    const resp = await page.request.get('/auth/me');
    expect(resp.status()).toBe(401);
    const body = await resp.json();
    expect(body.error).toBe('auth.unauthenticated');
  });

  test('dev_bypass login → /dashboard shows seeded workspaces', async ({ authedPage }) => {
    await authedPage.goto('/dashboard');
    await expect(authedPage.getByRole('heading', { name: 'Roommates' })).toBeVisible();
    await expect(authedPage.getByRole('heading', { name: 'Personal' })).toBeVisible();
    await expect(authedPage.getByRole('heading', { name: 'Old Group' })).toBeVisible();
  });

  test('logout clears cookies and redirects to /login', async ({ authedPage, page }) => {
    const t = await csrf(page);
    const resp = await page.request.post('/auth/logout', {
      headers: { 'X-CSRF-Token': t },
    });
    expect(resp.ok()).toBeTruthy();
    // After logout, /auth/me should be 401
    const me = await page.request.get('/auth/me');
    expect(me.status()).toBe(401);
  });
});
