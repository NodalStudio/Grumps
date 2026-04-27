import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

test.describe('Memory API', () => {
  test('list returns the 3 seeded pinned facts', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/memory`);
    expect(r.ok()).toBeTruthy();
    const entries = await r.json();
    expect(Array.isArray(entries)).toBe(true);
    expect(entries.length).toBeGreaterThanOrEqual(3);
    expect(entries.every((e: any) => typeof e.value === 'string')).toBe(true);
  });

  test('filter by kind=fact returns only facts', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/memory?kind=fact`);
    expect(r.ok()).toBeTruthy();
    const entries = await r.json();
    expect(entries.every((e: any) => e.kind === 'fact')).toBe(true);
  });

  test('CRUD round-trip', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/memory`, {
      headers: { 'X-CSRF-Token': t },
      data: { value: 'Test memory entry', kind: 'fact', pinned: false },
    });
    expect(created.status()).toBe(201);
    const entry = await created.json();
    expect(entry.id).toBeTruthy();
    expect(entry.value).toBe('Test memory entry');

    const got = await authedPage.request.get(`/api/w/${SLUG}/memory/${entry.id}`);
    expect(got.ok()).toBeTruthy();

    const updated = await authedPage.request.put(`/api/w/${SLUG}/memory/${entry.id}`, {
      headers: { 'X-CSRF-Token': t },
      data: { value: 'Updated value', pinned: true },
    });
    expect(updated.ok()).toBeTruthy();
    const after = await updated.json();
    expect(after.value).toBe('Updated value');
    expect(after.pinned).toBe(true);

    const deleted = await authedPage.request.delete(`/api/w/${SLUG}/memory/${entry.id}`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect(deleted.status()).toBe(204);

    const gone = await authedPage.request.get(`/api/w/${SLUG}/memory/${entry.id}`);
    expect(gone.status()).toBe(404);
  });

  test('expires_at in the past hides the entry from list', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/memory`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        value: 'Already expired secret',
        kind: 'fact',
        expires_at: '2020-01-01T00:00:00Z',
      },
    });
    expect(created.status()).toBe(201);
    const entry = await created.json();

    const list = await authedPage.request.get(`/api/w/${SLUG}/memory`);
    const items = await list.json();
    expect(items.find((m: any) => m.id === entry.id)).toBeUndefined();

    // Direct GET still works (route doesn't filter on single-row reads).
    const got = await authedPage.request.get(`/api/w/${SLUG}/memory/${entry.id}`);
    expect(got.ok()).toBeTruthy();

    await authedPage.request.delete(`/api/w/${SLUG}/memory/${entry.id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('expires_at is persisted', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/memory`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        value: 'TTL probe',
        kind: 'fact',
        expires_at: '2099-12-31T23:59:59Z',
      },
    });
    expect(created.status()).toBe(201);
    const entry = await created.json();
    expect(entry.expires_at).toContain('2099');

    await authedPage.request.delete(`/api/w/${SLUG}/memory/${entry.id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('related_member FK is persisted', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/memory`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        value: 'Bob loves coffee',
        kind: 'preference',
        related_member: 'm-bob',
      },
    });
    expect(created.status()).toBe(201);
    const entry = await created.json();
    expect(entry.related_member).toBe('m-bob');

    await authedPage.request.delete(`/api/w/${SLUG}/memory/${entry.id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('rejects empty value', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/memory`, {
      headers: { 'X-CSRF-Token': t },
      data: { value: '   ', kind: 'fact' },
    });
    expect(r.status()).toBe(400);
  });

  test('rejects mutation without CSRF', async ({ authedPage }) => {
    const r = await authedPage.request.post(`/api/w/${SLUG}/memory`, {
      data: { value: 'No CSRF', kind: 'fact' },
    });
    expect(r.status()).toBe(403);
  });
});

test.describe('Memory UI integration', () => {
  test('overview "What I know" widget renders pinned memories', async ({ authedPage }) => {
    await authedPage.goto(`/w/${SLUG}`);
    // Wait for the memory list to render (Suspense). We seeded 3 pinned facts
    // ("Rent is 1450 EUR…", "WiFi: GrumpsHouse…", "Bob handles trash…").
    await expect(authedPage.getByText(/Rent is 1450 EUR/i)).toBeVisible({ timeout: 10_000 });
  });
});
