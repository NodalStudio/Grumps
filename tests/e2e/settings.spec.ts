import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

test.describe('Workspace settings API', () => {
  test('PATCH /settings/locale accepts a supported locale', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.patch(`/api/w/${SLUG}/settings/locale`, {
      headers: { 'X-CSRF-Token': t },
      data: { locale: 'en' },
    });
    expect(r.ok()).toBeTruthy();
    const body = await r.json();
    expect(body.ok).toBe(true);
    expect(body.locale).toBe('en');
  });

  test('PATCH /settings/locale rejects an unknown locale', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.patch(`/api/w/${SLUG}/settings/locale`, {
      headers: { 'X-CSRF-Token': t },
      data: { locale: 'xx' },
    });
    expect(r.status()).toBe(400);
  });

  test('PATCH /settings/name updates and we can read the new name back', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const newName = 'Roommates Updated';
    const r = await authedPage.request.patch(`/api/w/${SLUG}/settings/name`, {
      headers: { 'X-CSRF-Token': t },
      data: { name: newName },
    });
    expect(r.ok()).toBeTruthy();

    const me = await authedPage.request.get('/auth/me');
    const session = await me.json();
    const ws = session.workspaces.find((w: any) => w.slug === SLUG);
    expect(ws?.name).toBe(newName);

    // Restore for idempotence.
    await authedPage.request.patch(`/api/w/${SLUG}/settings/name`, {
      headers: { 'X-CSRF-Token': t },
      data: { name: 'Roommates' },
    });
  });

  test('PATCH /settings/name rejects empty', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.patch(`/api/w/${SLUG}/settings/name`, {
      headers: { 'X-CSRF-Token': t },
      data: { name: '' },
    });
    expect(r.status()).toBe(400);
  });

  test('PATCH /settings/name rejects > 80 chars', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.patch(`/api/w/${SLUG}/settings/name`, {
      headers: { 'X-CSRF-Token': t },
      data: { name: 'x'.repeat(81) },
    });
    expect(r.status()).toBe(400);
  });

  test('PATCH /api/me rejects empty display_name', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.patch(`/api/me`, {
      headers: { 'X-CSRF-Token': t },
      data: { display_name: '   ' },
    });
    expect(r.status()).toBe(400);
  });

  test('PATCH /api/me rejects > 80 char display_name', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.patch(`/api/me`, {
      headers: { 'X-CSRF-Token': t },
      data: { display_name: 'x'.repeat(81) },
    });
    expect(r.status()).toBe(400);
  });

  test('PATCH /api/me updates display_name', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.patch(`/api/me`, {
      headers: { 'X-CSRF-Token': t },
      data: { display_name: 'Tester Renamed' },
    });
    expect(r.ok()).toBeTruthy();

    const me = await authedPage.request.get('/auth/me');
    const session = await me.json();
    expect(session.display_name).toBe('Tester Renamed');

    // Restore.
    await authedPage.request.patch(`/api/me`, {
      headers: { 'X-CSRF-Token': t },
      data: { display_name: 'Tester' },
    });
  });
});
