import { test, expect } from './fixtures';

const SLUG = 'test-grp1';

test.describe('Members API', () => {
  test('lists 5 seeded members of Roommates', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/members`);
    expect(r.ok()).toBeTruthy();
    const members = await r.json();
    expect(members.length).toBe(5);
    const names = members.map((m: any) => m.display_name).sort();
    expect(names).toEqual(['Bob', 'Charlie', 'Diana', 'Erik', 'Tester']);
  });

  test('Tester is the admin', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/members`);
    const members = await r.json();
    const tester = members.find((m: any) => m.display_name === 'Tester');
    expect(tester?.role).toBe('admin');
  });

  test('non-members get 403', async ({ authedPage }) => {
    // No workspace test-foo exists; should be 403/404 not 200.
    const r = await authedPage.request.get(`/api/w/test-foo/members`);
    expect([403, 404]).toContain(r.status());
  });

  test('DM workspace has 1 member', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/test-dm1/members`);
    expect(r.ok()).toBeTruthy();
    const members = await r.json();
    expect(members.length).toBe(1);
    expect(members[0].display_name).toBe('Tester');
  });
});
