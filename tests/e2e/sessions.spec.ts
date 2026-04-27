import { test, expect, csrf } from './fixtures';

test.describe('Sessions API', () => {
  test('GET /auth/sessions lists at least the current session', async ({ authedPage }) => {
    const r = await authedPage.request.get('/auth/sessions');
    expect(r.ok()).toBeTruthy();
    const body = await r.json();
    expect(Array.isArray(body.sessions)).toBe(true);
    expect(body.sessions.length).toBeGreaterThanOrEqual(1);
    expect(body.sessions.some((s: any) => s.is_current)).toBe(true);
  });

  test('DELETE /auth/sessions/<id> revokes a specific session by id', async ({ authedPage, browser }) => {
    const t = await csrf(authedPage);

    // Open a second browser context (= a second session) for the same user.
    const ctx2 = await browser.newContext();
    const page2 = await ctx2.newPage();
    const r2 = await page2.request.post('http://127.0.0.1:8787/auth/telegram/verify', {
      headers: { 'Content-Type': 'application/json' },
      data: {
        id: 6108569905, first_name: 'Tester',
        auth_date: Math.floor(Date.now() / 1000),
        hash: 'unused', dev_bypass: true,
      },
    });
    expect(r2.ok()).toBeTruthy();

    // List sessions from session #1; find a non-current one (= session #2).
    const list = await authedPage.request.get('/auth/sessions');
    const body = await list.json();
    const otherSid = body.sessions.find((s: any) => !s.is_current)?.id;
    expect(otherSid).toBeTruthy();

    // Revoke that specific session.
    const del = await authedPage.request.delete(`/auth/sessions/${otherSid}`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect(del.ok()).toBeTruthy();

    // Session #1 (the one in authedPage) is still alive.
    const me = await authedPage.request.get('/auth/me');
    expect(me.status()).toBe(200);

    // Session #2 should now reject /auth/me.
    const me2 = await page2.request.get('http://127.0.0.1:8787/auth/me');
    expect(me2.status()).toBe(401);

    await ctx2.close();
  });

  test('DELETE /auth/sessions/<unknown-id> returns 404', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.delete('/auth/sessions/00000000-0000-0000-0000-000000000000', {
      headers: { 'X-CSRF-Token': t },
    });
    expect(r.status()).toBe(404);
  });

  test('POST /auth/sessions/revoke-all-others does not revoke current session', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post('/auth/sessions/revoke-all', {
      headers: { 'X-CSRF-Token': t },
    });
    expect(r.ok()).toBeTruthy();

    // Current session should still work.
    const me = await authedPage.request.get('/auth/me');
    expect(me.status()).toBe(200);
  });
});
