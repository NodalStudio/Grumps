import { test, expect } from './fixtures';

const SLUG = 'test-grp1';

test.describe('Export endpoints', () => {
  test('export todos as JSON', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/export/todos?format=json`);
    expect(r.ok()).toBeTruthy();
    expect(r.headers()['content-type']).toContain('application/json');
    expect(r.headers()['content-disposition']).toContain('todos.json');
    const todos = await r.json();
    expect(Array.isArray(todos)).toBe(true);
    expect(todos.some((t: any) => t.title === 'Buy milk')).toBe(true);
  });

  test('export todos as CSV', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/export/todos?format=csv`);
    expect(r.ok()).toBeTruthy();
    expect(r.headers()['content-type']).toContain('text/csv');
    expect(r.headers()['content-disposition']).toContain('todos.csv');
    const text = await r.text();
    expect(text.startsWith('seq_num,title,status,')).toBe(true);
    expect(text).toContain('Buy milk');
  });

  test('export notes as JSON', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/export/notes?format=json`);
    expect(r.ok()).toBeTruthy();
    expect(r.headers()['content-type']).toContain('application/json');
    const notes = await r.json();
    expect(Array.isArray(notes)).toBe(true);
  });

  test('export rejects non-members', async ({ authedPage }) => {
    const r = await authedPage.request.get('/api/w/some-other-tenant/export/todos');
    expect([403, 404]).toContain(r.status());
  });
});
