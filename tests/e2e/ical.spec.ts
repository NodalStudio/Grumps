import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

test.describe('iCal token API', () => {
  test('create + delete iCal token', async ({ authedPage }) => {
    const t = await csrf(authedPage);

    const created = await authedPage.request.post(`/api/w/${SLUG}/calendar/ical-token`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect(created.ok()).toBeTruthy();
    const body = await created.json();
    expect(typeof body.token).toBe('string');
    expect(body.token.length).toBeGreaterThan(8);

    const deleted = await authedPage.request.delete(`/api/w/${SLUG}/calendar/ical-token`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect(deleted.ok()).toBeTruthy();
  });

  test('mutation requires CSRF', async ({ authedPage }) => {
    const r = await authedPage.request.post(`/api/w/${SLUG}/calendar/ical-token`);
    expect(r.status()).toBe(403);
  });
});
