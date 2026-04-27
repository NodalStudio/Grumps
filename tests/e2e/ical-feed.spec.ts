import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

test.describe('iCal feed', () => {
  test('feed without token returns error', async ({ authedPage }) => {
    const r = await authedPage.request.get(`http://127.0.0.1:8787/cal/${SLUG}.ics`);
    // Worker returns 500 on missing param (treated as RustError); accept that
    // or a 4xx — we just shouldn't get a successful 200.
    expect(r.ok()).toBe(false);
  });

  test('feed with bad token returns 4xx/5xx', async ({ authedPage }) => {
    const r = await authedPage.request.get(`http://127.0.0.1:8787/cal/${SLUG}.ics?t=bogus`);
    expect(r.ok()).toBe(false);
  });

  test('feed with valid token returns 200 + iCalendar body', async ({ authedPage }) => {
    const t = await csrf(authedPage);

    // Mint a fresh token.
    const created = await authedPage.request.post(`/api/w/${SLUG}/calendar/ical-token`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect(created.ok()).toBeTruthy();
    const { token } = await created.json();
    expect(typeof token).toBe('string');

    const r = await authedPage.request.get(`http://127.0.0.1:8787/cal/${SLUG}.ics?t=${encodeURIComponent(token)}`);
    expect(r.ok()).toBeTruthy();
    const body = await r.text();
    expect(body.startsWith('BEGIN:VCALENDAR')).toBe(true);
    expect(body).toContain('END:VCALENDAR');

    // After revocation, the same token must stop working.
    const deleted = await authedPage.request.delete(`/api/w/${SLUG}/calendar/ical-token`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect(deleted.ok()).toBeTruthy();

    const after = await authedPage.request.get(`http://127.0.0.1:8787/cal/${SLUG}.ics?t=${encodeURIComponent(token)}`);
    expect(after.ok()).toBe(false);
  });
});
