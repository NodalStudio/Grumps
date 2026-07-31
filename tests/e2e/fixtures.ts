import {
  test as base,
  expect,
  type Page,
  type APIRequestContext,
  type BrowserContext,
  type Cookie,
} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

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

// `POST /auth/telegram/verify` is rate-limited to 10 req/min/IP — bounds
// session-table growth under spam (see `check_rate_limit` in
// crates/worker/src/routes/auth.rs), applied unconditionally including to
// the dev_bypass shortcut. Correct production behavior, but this suite has
// 100+ tests that each want an authenticated page, and a real login per
// test would blow the limit almost immediately. So: log in for real exactly
// ONCE per run, then clone that session's cookies onto every later test's
// fresh context — no further requests to the rate-limited endpoint. A test
// that needs to end or replace its OWN session (logout, session revocation
// of the *current* session) must not use this shared session — call
// `loginAs` directly for a disposable one instead (see auth.spec.ts's
// logout test).
//
// Cached on disk, not just in a module-level variable: Playwright runs each
// spec file's tests through fresh module instances (in-process caching alone
// still re-triggered a real login per file, ~1 per test — verified against
// the worker's request log), but every file agrees on this same path.
const AUTH_CACHE_PATH = path.join(__dirname, '..', '.auth-cache.json');
let sharedSessionCookies: Cookie[] | null = null;

async function useSharedSession(context: BrowserContext, page: Page): Promise<void> {
  if (!sharedSessionCookies) {
    sharedSessionCookies = readCachedCookies();
  }
  if (!sharedSessionCookies) {
    await loginAs(page);
    sharedSessionCookies = await context.cookies();
    fs.writeFileSync(AUTH_CACHE_PATH, JSON.stringify(sharedSessionCookies));
    return;
  }
  await context.addCookies(sharedSessionCookies);
}

function readCachedCookies(): Cookie[] | null {
  try {
    return JSON.parse(fs.readFileSync(AUTH_CACHE_PATH, 'utf8'));
  } catch {
    return null;
  }
}

export const test = base.extend<{ authedPage: Page }>({
  authedPage: async ({ page }, use) => {
    await page.goto('/login');
    await useSharedSession(page.context(), page);
    await use(page);
  },
});

export { expect };
