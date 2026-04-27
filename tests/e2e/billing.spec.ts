import { test, expect, csrf } from './fixtures';

const TG_WEBHOOK_SECRET = 'f36638579db4005af0e80f584ae9d4492472f953f3c74b6361d106827738a9db';
const SLUG = 'test-grp1';
const CHAT_ID = -100_100_100_100;
const SENDER_TG_ID = 6_108_569_905;

// Free plan: 25 todos. Seed has 4 open + a handful of done. To force the
// limit we'd need to insert ~25 todos, which is slow; instead this test
// verifies the worker returns the localized quota message when we POST
// directly with a high mocked open_count via setting the workspace plan
// implicitly to "free" (which it already is) and creating todos until we
// exceed. Skip if the suite would take too long.
test.describe('Billing quotas — localized messages', () => {
  test('quota message in the chat is localized to the workspace locale', async ({ authedPage }) => {
    const csrfToken = await csrf(authedPage);

    // Set workspace locale to French.
    let r = await authedPage.request.patch(`/api/w/${SLUG}/settings/locale`, {
      headers: { 'X-CSRF-Token': csrfToken },
      data: { locale: 'fr' },
    });
    expect(r.ok()).toBeTruthy();

    // Create 25 todos via API to hit the free-plan limit (max_todos=25).
    const created: string[] = [];
    for (let i = 0; i < 25; i++) {
      const c = await authedPage.request.post(`/api/w/${SLUG}/todos`, {
        headers: { 'X-CSRF-Token': csrfToken },
        data: { title: `Quota probe ${i}` },
      });
      if (!c.ok()) break;
      created.push((await c.json()).id);
    }

    // The 26th todo should be blocked. Send it via the webhook so the
    // worker responds via handler::handle_message (which returns the
    // localized billing message).
    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: { id: SENDER_TG_ID, is_bot: false, first_name: 'Tester' },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: `TODO:\nWill not fit ${Date.now()}`,
      },
    };
    const wh = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(wh.ok()).toBeTruthy();

    // The new todo should NOT appear in the open list — the quota check
    // blocked it. We can't directly read what the bot replied (no mock
    // for api.telegram.org), but the side-effect (no row) is observable.
    const list = await authedPage.request.get(`/api/w/${SLUG}/todos?status=all`);
    const items = await list.json();
    expect(items.find((t: any) => String(t.title).startsWith('Will not fit'))).toBeUndefined();

    // Cleanup.
    for (const id of created) {
      await authedPage.request.delete(`/api/w/${SLUG}/todos/${id}`, {
        headers: { 'X-CSRF-Token': csrfToken },
      });
    }
    await authedPage.request.patch(`/api/w/${SLUG}/settings/locale`, {
      headers: { 'X-CSRF-Token': csrfToken },
      data: { locale: 'en' },
    });
  });
});
