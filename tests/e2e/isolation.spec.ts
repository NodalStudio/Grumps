import { test, expect } from './fixtures';

// Cross-workspace isolation: a user authenticated to test-grp1 must not be
// able to read another tenant's data, even if they know its slug.

test.describe('Workspace isolation', () => {
  test('GET /api/w/<unknown-slug> returns 403 or 404', async ({ authedPage }) => {
    const r = await authedPage.request.get('/api/w/some-other-tenant/todos?status=open');
    expect([403, 404]).toContain(r.status());
  });

  test('GET /api/w/<unknown-slug>/notes returns 403 or 404', async ({ authedPage }) => {
    const r = await authedPage.request.get('/api/w/some-other-tenant/notes');
    expect([403, 404]).toContain(r.status());
  });

  test('GET /api/w/<unknown-slug>/members returns 403 or 404', async ({ authedPage }) => {
    const r = await authedPage.request.get('/api/w/some-other-tenant/members');
    expect([403, 404]).toContain(r.status());
  });

  test('user can access all 3 of their seeded workspaces', async ({ authedPage }) => {
    for (const slug of ['test-grp1', 'test-dm1', 'test-arc1']) {
      const r = await authedPage.request.get(`/api/w/${slug}`);
      expect(r.ok(), `failed for ${slug}: ${r.status()}`).toBeTruthy();
    }
  });
});
