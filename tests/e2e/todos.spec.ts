import { test, expect, csrf } from './fixtures';

test.describe('Todos', () => {
  test('lists 5 open seeded todos by default', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1/todos');
    await expect(authedPage.getByText('Buy milk')).toBeVisible({ timeout: 10_000 });
    await expect(authedPage.getByText('Take out the trash')).toBeVisible();
    await expect(authedPage.getByText('Clean the kitchen')).toBeVisible();
    await expect(authedPage.getByText('Buy more coffee beans')).toBeVisible();
    await expect(authedPage.getByText('Call the plumber')).toBeVisible();
    // "Pay the rent" is done — not in the open filter.
    await expect(authedPage.getByText('Pay the rent')).toHaveCount(0);
  });

  test('filter "Done" reveals completed todos', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1/todos');
    await authedPage.getByRole('button', { name: 'Done', exact: true }).click();
    await expect(authedPage.getByText('Pay the rent')).toBeVisible();
    await expect(authedPage.getByText('Buy milk')).toHaveCount(0);
  });

  test('filter "All" shows every todo', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1/todos');
    await authedPage.getByRole('button', { name: 'All', exact: true }).click();
    await expect(authedPage.getByText('Buy milk')).toBeVisible();
    await expect(authedPage.getByText('Pay the rent')).toBeVisible();
  });

  test('checkbox toggle marks todo done with animation, then row disappears in Open filter', async ({ authedPage }) => {
    // Create a dedicated todo so the test is idempotent (doesn't mutate seeded state).
    const csrfToken = await csrf(authedPage);
    const created = await authedPage.request.post('/api/w/test-grp1/todos', {
      headers: { 'X-CSRF-Token': csrfToken },
      data: { title: 'Toggle target' },
    });
    const { id } = await created.json();

    await authedPage.goto('/w/test-grp1/todos');
    const row = authedPage.locator('.todo-row').filter({ hasText: 'Toggle target' });
    await expect(row).toBeVisible({ timeout: 10_000 });
    await row.locator('.todo-checkbox').click();
    await expect(authedPage.getByText('Toggle target')).toHaveCount(0, { timeout: 5_000 });

    // Cleanup.
    await authedPage.request.delete(`/api/w/test-grp1/todos/${id}`, {
      headers: { 'X-CSRF-Token': csrfToken },
    });
  });

  test('+ Add todo button focuses the inline input', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1/todos');
    await authedPage.getByRole('button', { name: /Add todo/i }).click();
    const input = authedPage.locator('#new-todo-input');
    await expect(input).toBeFocused();
  });

  test('typing + Enter creates a new todo', async ({ authedPage }) => {
    await authedPage.goto('/w/test-grp1/todos');
    const input = authedPage.locator('#new-todo-input');
    await input.fill('Test from Playwright');
    await input.press('Enter');
    await expect(authedPage.getByText('Test from Playwright')).toBeVisible({ timeout: 5_000 });
    // Cleanup so the test is idempotent.
    const csrfToken = await csrf(authedPage);
    const todos = await authedPage.request.get('/api/w/test-grp1/todos?status=open');
    const json = await todos.json();
    const created = (json as Array<{ id: string; title: string }>).find(t => t.title === 'Test from Playwright');
    if (created) {
      await authedPage.request.delete(`/api/w/test-grp1/todos/${created.id}`, {
        headers: { 'X-CSRF-Token': csrfToken },
      });
    }
  });
});
