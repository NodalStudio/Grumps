import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

// Verify malformed JSON or missing required fields produce a 400 with a
// proper error code rather than a generic 500.
test.describe('Malformed input → 400 (not 500)', () => {
  test('POST /api/w/<slug>/notes with empty body → 400', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/notes`, {
      headers: { 'X-CSRF-Token': t },
      data: {},
    });
    expect(r.status()).toBe(400);
    expect((await r.json()).error).toBe('bad_request');
  });

  test('PUT /api/w/<slug>/notes/<id> with empty body → 400', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.put(`/api/w/${SLUG}/notes/some-id`, {
      headers: { 'X-CSRF-Token': t },
      data: {},
    });
    expect(r.status()).toBe(400);
  });

  test('POST /api/w/<slug>/todos with empty body → 400', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: {},
    });
    expect(r.status()).toBe(400);
  });

  test('POST /api/w/<slug>/memory with empty body → 400', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/memory`, {
      headers: { 'X-CSRF-Token': t },
      data: {},
    });
    expect(r.status()).toBe(400);
  });

  test('POST /api/w/<slug>/events with empty body → 400', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: {},
    });
    expect(r.status()).toBe(400);
  });

  test('POST /api/w/<slug>/scheduled with empty body → 400', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/scheduled`, {
      headers: { 'X-CSRF-Token': t },
      data: {},
    });
    expect(r.status()).toBe(400);
  });
});
