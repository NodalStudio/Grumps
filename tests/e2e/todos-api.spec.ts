import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

async function listOpen(page: any, t: string) {
  const r = await page.request.get(`/api/w/${SLUG}/todos?status=open`, {
    headers: { 'X-CSRF-Token': t },
  });
  expect(r.ok()).toBeTruthy();
  return r.json() as Promise<Array<{ id: string; title: string; status: string; assigned_name?: string; priority?: number }>>;
}

async function deleteTodo(page: any, t: string, id: string) {
  const r = await page.request.delete(`/api/w/${SLUG}/todos/${id}`, {
    headers: { 'X-CSRF-Token': t },
  });
  expect(r.ok()).toBeTruthy();
}

test.describe('Todos API', () => {
  test('create + list + delete', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'API created todo' },
    });
    expect(created.ok()).toBeTruthy();
    const { id } = await created.json();

    const todos = await listOpen(authedPage, t);
    expect(todos.find(x => x.id === id)?.title).toBe('API created todo');

    await deleteTodo(authedPage, t, id);
    const after = await listOpen(authedPage, t);
    expect(after.find(x => x.id === id)).toBeUndefined();
  });

  test('create with assignee', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Assigned todo', assigned_to: 'm-bob', assigned_name: 'Bob' },
    });
    expect(created.ok()).toBeTruthy();
    const { id } = await created.json();

    // /todos defaults to filtering by current user's assignee. Use ?assignee=m-bob.
    const r = await authedPage.request.get(`/api/w/${SLUG}/todos?status=open&assignee=m-bob`, {
      headers: { 'X-CSRF-Token': t },
    });
    const todos = await r.json();
    const got = todos.find((x: any) => x.id === id);
    expect(got?.assigned_name).toBe('Bob');

    await deleteTodo(authedPage, t, id);
  });

  test('patch status open → done removes from open filter', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Will be marked done' },
    });
    const { id } = await created.json();

    const patch = await authedPage.request.patch(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
      data: { status: 'done' },
    });
    expect(patch.ok()).toBeTruthy();

    const open = await listOpen(authedPage, t);
    expect(open.find(x => x.id === id)).toBeUndefined();

    const doneR = await authedPage.request.get(`/api/w/${SLUG}/todos?status=done`, {
      headers: { 'X-CSRF-Token': t },
    });
    const done = await doneR.json();
    expect(done.find((x: any) => x.id === id)?.status).toBe('done');

    await deleteTodo(authedPage, t, id);
  });

  test('patch reassigns todo', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Reassign me', assigned_to: 'm-alice', assigned_name: 'Tester' },
    });
    const { id } = await created.json();

    const patch = await authedPage.request.patch(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
      data: { assigned_to: 'm-charlie', assigned_name: 'Charlie' },
    });
    expect(patch.ok()).toBeTruthy();

    const r = await authedPage.request.get(`/api/w/${SLUG}/todos?status=open&assignee=m-charlie`, {
      headers: { 'X-CSRF-Token': t },
    });
    const todos = await r.json();
    expect(todos.find((x: any) => x.id === id)?.assigned_name).toBe('Charlie');

    await deleteTodo(authedPage, t, id);
  });

  test('patch updates title', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Original title' },
    });
    const { id } = await created.json();

    await authedPage.request.patch(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Updated title' },
    });

    const open = await listOpen(authedPage, t);
    expect(open.find(x => x.id === id)?.title).toBe('Updated title');

    await deleteTodo(authedPage, t, id);
  });

  test('rejects priority outside 1..3 on POST', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Bad priority', priority: 99 },
    });
    expect(r.status()).toBe(400);
  });

  test('rejects unknown status on PATCH', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Status probe' },
    });
    const { id } = await created.json();

    const r = await authedPage.request.patch(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
      data: { status: 'invalid-status' },
    });
    expect(r.status()).toBe(400);

    await authedPage.request.delete(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('rejects empty title', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: '   ' },
    });
    expect(r.status()).toBe(400);
  });

  test('rejects title longer than 500 chars on POST', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'x'.repeat(501) },
    });
    expect(r.status()).toBe(400);
  });

  test('rejects title longer than 500 chars on PATCH', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'Length probe' },
    });
    const { id } = await created.json();

    const r = await authedPage.request.patch(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'x'.repeat(501) },
    });
    expect(r.status()).toBe(400);

    await authedPage.request.delete(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('rejects mutation without CSRF', async ({ authedPage }) => {
    const r = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      data: { title: 'No CSRF' },
    });
    expect(r.status()).toBe(403);
  });

  test('rejects access to a workspace the user is not a member of', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.get(`/api/w/does-not-exist/todos?status=open`, {
      headers: { 'X-CSRF-Token': t },
    });
    expect([403, 404]).toContain(r.status());
  });
});
