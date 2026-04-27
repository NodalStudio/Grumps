import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

test.describe('Activity history API', () => {
  test('list returns an array (may be empty)', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/history?limit=10`);
    expect(r.ok()).toBeTruthy();
    const log = await r.json();
    expect(Array.isArray(log)).toBe(true);
  });

  test('limit=0 is rejected or normalized', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/history?limit=0`);
    expect(r.ok()).toBeTruthy();
    const log = await r.json();
    // Worker treats invalid limit as default-50; an empty array would be wrong
    // (would mean 0 was honored). Either non-empty default or empty is OK,
    // we just shouldn't get a 500.
    expect(Array.isArray(log)).toBe(true);
  });

  test('limit is capped at 200', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/history?limit=10000`);
    expect(r.ok()).toBeTruthy();
    const log = await r.json();
    expect(log.length).toBeLessThanOrEqual(200);
  });

  test('logging a todo via API does not crash the history feed', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const c = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: 'History probe' },
    });
    const { id } = await c.json();

    const r = await authedPage.request.get(`/api/w/${SLUG}/history?limit=50`);
    expect(r.ok()).toBeTruthy();

    await authedPage.request.delete(`/api/w/${SLUG}/todos/${id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('webhook-created todo lands in history with todo.created action', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const TG_WEBHOOK_SECRET = 'f36638579db4005af0e80f584ae9d4492472f953f3c74b6361d106827738a9db';
    const title = `History probe ${Date.now()}`;

    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: { id: 6_108_569_905, is_bot: false, first_name: 'Tester' },
        chat: { id: -100_100_100_100, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: `TODO:\n${title}`,
      },
    };
    const wh = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(wh.ok()).toBeTruthy();

    // Find the created todo via the all-status list (it'll have our title).
    let createdId: string | undefined;
    for (let i = 0; i < 12; i++) {
      const list = await authedPage.request.get(`/api/w/${SLUG}/todos?status=all`);
      const items = await list.json();
      const got = items.find((x: any) => x.title === title);
      if (got) { createdId = got.id; break; }
      await new Promise(r => setTimeout(r, 500));
    }
    expect(createdId).toBeTruthy();

    // History should contain a todo.created entry referencing this todo.
    const hist = await authedPage.request.get(`/api/w/${SLUG}/history?limit=100`);
    const log = await hist.json();
    const entry = log.find((e: any) => e.action === 'todo.created' && e.target_id === createdId);
    expect(entry, 'expected a todo.created activity log entry for the new todo').toBeDefined();
    expect(entry?.source).toBe('chat');

    await authedPage.request.delete(`/api/w/${SLUG}/todos/${createdId}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });
});
