import { test as base, expect, type Page, type APIRequestContext } from '@playwright/test';

/**
 * Auth bypass: POST to /auth/telegram/verify with dev_bypass=true. Server
 * accepts iff GRUMPS_DEV_AUTH_BYPASS secret is set (only in dev). Returns
 * cookies that the browser context will reuse for the rest of the test.
 */
export async function loginAs(page: Page, telegramId: number = 6108569905, firstName = 'Tester') {
  const resp = await page.request.post('/auth/telegram/verify', {
    headers: { 'Content-Type': 'application/json', Origin: 'http://localhost:8080' },
    data: {
      id: telegramId,
      first_name: firstName,
      auth_date: Math.floor(Date.now() / 1000),
      hash: 'unused',
      dev_bypass: true,
    },
  });
  expect(resp.ok(), `dev_bypass login failed: ${resp.status()}`).toBeTruthy();
  return resp.json();
}

/**
 * Read the current CSRF token from the browser cookie jar (synced with
 * the worker's Set-Cookie). Used for direct API calls bypassing the SPA.
 */
export async function csrf(page: Page): Promise<string> {
  return page.evaluate(() => {
    const m = document.cookie.match(/(?:^|;\s*)grumps_csrf=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : '';
  });
}

export const test = base.extend<{ authedPage: Page }>({
  authedPage: async ({ page }, use) => {
    await page.goto('/login');
    await loginAs(page);
    await use(page);
  },
});

export { expect };
