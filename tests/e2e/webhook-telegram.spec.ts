import { test, expect } from './fixtures';

const TG_WEBHOOK_SECRET = 'f36638579db4005af0e80f584ae9d4492472f953f3c74b6361d106827738a9db';

test.describe('Telegram webhook', () => {
  test('rejects request without X-Telegram-Bot-Api-Secret-Token header', async ({ page }) => {
    const r = await page.request.post('http://127.0.0.1:8787/webhook/telegram', {
      data: { update_id: 1 },
    });
    // Worker treats missing header as a hard error (500 in dev / 403 in prod).
    expect([400, 403, 500]).toContain(r.status());
  });

  test('rejects request with wrong secret', async ({ page }) => {
    const r = await page.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': 'wrong-secret' },
      data: { update_id: 1 },
    });
    expect(r.status()).toBe(403);
  });

  test('accepts message with valid secret (no-op for unknown chat)', async ({ page }) => {
    // Sending a message from a chat we haven't seen — worker should not crash.
    const update = {
      update_id: 999_001,
      message: {
        message_id: 1,
        from: { id: 999_999_001, is_bot: false, first_name: 'Random' },
        chat: { id: -100_999_999_001, type: 'group', title: 'Random group' },
        date: Math.floor(Date.now() / 1000),
        text: 'hello bot',
      },
    };
    const r = await page.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(r.ok()).toBeTruthy();
  });
});
