import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

test.describe('Notes API', () => {
  test('list returns the 2 seeded notes', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/notes`);
    expect(r.ok()).toBeTruthy();
    const notes = await r.json();
    expect(Array.isArray(notes)).toBe(true);
    expect(notes.length).toBeGreaterThanOrEqual(2);
  });

  test('create + get + update + delete round-trip', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/notes`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Test note', content: 'Some content body.' },
    });
    expect(created.ok()).toBeTruthy();
    const { id } = await created.json();
    expect(id).toBeTruthy();

    const got = await authedPage.request.get(`/api/w/${SLUG}/notes/${id}`);
    expect(got.ok()).toBeTruthy();
    const note = await got.json();
    expect(note.title).toBe('Test note');
    expect(note.content).toBe('Some content body.');

    const updated = await authedPage.request.put(`/api/w/${SLUG}/notes/${id}`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Updated title', content: 'Updated body.' },
    });
    expect(updated.ok()).toBeTruthy();

    const got2 = await authedPage.request.get(`/api/w/${SLUG}/notes/${id}`);
    const note2 = await got2.json();
    expect(note2.title).toBe('Updated title');
    expect(note2.content).toBe('Updated body.');

    const deleted = await authedPage.request.delete(`/api/w/${SLUG}/notes/${id}`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect(deleted.ok()).toBeTruthy();

    const got3 = await authedPage.request.get(`/api/w/${SLUG}/notes/${id}`);
    expect(got3.status()).toBe(404);
  });

  test('mutation requires CSRF', async ({ authedPage }) => {
    const r = await authedPage.request.post(`/api/w/${SLUG}/notes`, {
      data: { title: 'No CSRF', content: '...' },
    });
    expect(r.status()).toBe(403);
  });
});

test.describe('Notes UI', () => {
  test('Notes page lists seeded notes', async ({ authedPage }) => {
    await authedPage.goto(`/w/${SLUG}/notes`);
    // Two seeded notes — assert at least two list items.
    await expect(authedPage.locator('h2').first()).toBeVisible({ timeout: 10_000 });
    // Don't depend on a specific seeded title (English-only seeds may be edited);
    // just ensure list is not empty.
    const links = authedPage.locator('a[href*="/notes/"]');
    await expect.poll(async () => await links.count()).toBeGreaterThan(0);
  });

  test('Note editor persists edits across reload', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    // Create a fresh note so we have a known id to edit.
    const created = await authedPage.request.post(`/api/w/${SLUG}/notes`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Editor target', content: 'Initial body.' },
    });
    const { id } = await created.json();

    await authedPage.goto(`/w/${SLUG}/notes/${id}?lang=en`);
    await expect(authedPage.locator('h2').first()).toBeVisible({ timeout: 10_000 });

    // Toggle into edit mode, type new content, then click Save.
    await authedPage.getByRole('button', { name: 'Edit', exact: true }).click();
    await authedPage.locator('textarea').fill('Updated by Playwright.');
    await authedPage.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(authedPage.getByText('Saved.', { exact: true })).toBeVisible({ timeout: 5_000 });

    // Reload — server should now return the updated content.
    await authedPage.reload();
    const got = await authedPage.request.get(`/api/w/${SLUG}/notes/${id}`);
    const detail = await got.json();
    expect(detail.content).toBe('Updated by Playwright.');

    // Cleanup.
    await authedPage.request.delete(`/api/w/${SLUG}/notes/${id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });
});
