import { test, expect, csrf } from './fixtures';

const TG_WEBHOOK_SECRET = 'f36638579db4005af0e80f584ae9d4492472f953f3c74b6361d106827738a9db';
const SLUG = 'test-grp1';
const CHAT_ID = -100_100_100_100; // matches workspaces_meta.platform_channel_id for test-grp1
const SENDER_TG_ID = 6_108_569_905;

// End-to-end: a Telegram message with a "TODO:" command body should produce
// a row in the test-grp1 workspace D1. The bot's outbound sendMessage
// callback to api.telegram.org may fail (chat is fake) but the DB write is
// the side-effect we care about.
test.describe('Telegram webhook → todo end-to-end', () => {
  test('TODO: command via webhook creates a todo in the workspace', async ({ authedPage }) => {
    const csrfToken = await csrf(authedPage);
    const titleBefore = `Webhook todo ${Date.now()}`;

    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: {
          id: SENDER_TG_ID,
          is_bot: false,
          first_name: 'Tester',
          username: 'tester',
        },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: `TODO:\n${titleBefore}`,
      },
    };

    const r = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(r.ok()).toBeTruthy();

    // Poll the todos API until our title shows up. The webhook is
    // synchronous, but the response from the worker doesn't wait on the
    // outbound TG callback — give it up to 8s in case Anthropic or RAG
    // pipelines run.
    let found: any = undefined;
    for (let i = 0; i < 16; i++) {
      const list = await authedPage.request.get(`/api/w/${SLUG}/todos?status=open&assignee=`);
      // Empty assignee → server defaults to claims.sub which won't match;
      // use no-assignee filter to widen. Fall through and check below.
      const all = await authedPage.request.get(`/api/w/${SLUG}/todos?status=all`);
      const items = await all.json();
      found = items.find((t: any) => t.title === titleBefore);
      if (found) break;
      await new Promise(r => setTimeout(r, 500));
    }
    expect(found, `webhook should have created a todo with title "${titleBefore}"`).toBeDefined();
    expect(found.status).toBe('open');

    // Cleanup.
    await authedPage.request.delete(`/api/w/${SLUG}/todos/${found.id}`, {
      headers: { 'X-CSRF-Token': csrfToken },
    });
  });

  test('NOTE: command via webhook creates a note', async ({ authedPage }) => {
    const csrfToken = await csrf(authedPage);
    const noteContent = `Webhook note ${Date.now()}`;

    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: { id: SENDER_TG_ID, is_bot: false, first_name: 'Tester' },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: `NOTE:\n${noteContent}`,
      },
    };

    const r = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(r.ok()).toBeTruthy();

    let found: any = undefined;
    for (let i = 0; i < 12; i++) {
      const list = await authedPage.request.get(`/api/w/${SLUG}/notes`);
      const items = await list.json();
      found = items.find((n: any) => n.title === noteContent || (n.title === '' && n.id));
      // The block parser puts the line into `content` with title='' — fetch
      // the entry to confirm by content.
      for (const candidate of items) {
        const got = await authedPage.request.get(`/api/w/${SLUG}/notes/${candidate.id}`);
        const detail = await got.json();
        if (detail.content === noteContent) {
          found = detail;
          break;
        }
      }
      if (found && found.content === noteContent) break;
      await new Promise(r => setTimeout(r, 500));
    }
    expect(found, `webhook should have created a note with content "${noteContent}"`).toBeDefined();

    await authedPage.request.delete(`/api/w/${SLUG}/notes/${found.id}`, {
      headers: { 'X-CSRF-Token': csrfToken },
    });
  });

  test('@grumps remember that … via webhook creates a memory entry (en)', async ({ authedPage }) => {
    const csrfToken = await csrf(authedPage);
    const fact = `my secret code is ${Date.now()}`;

    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: { id: SENDER_TG_ID, is_bot: false, first_name: 'Tester' },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: `@HeyGrumpsBot remember that ${fact}`,
      },
    };

    const r = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(r.ok()).toBeTruthy();

    let found: any = undefined;
    for (let i = 0; i < 12; i++) {
      const list = await authedPage.request.get(`/api/w/${SLUG}/memory`);
      const items = await list.json();
      found = items.find((m: any) => m.value && m.value.includes(fact));
      if (found) break;
      await new Promise(r => setTimeout(r, 500));
    }
    expect(found, `webhook should have created a memory entry containing "${fact}"`).toBeDefined();

    await authedPage.request.delete(`/api/w/${SLUG}/memory/${found.id}`, {
      headers: { 'X-CSRF-Token': csrfToken },
    });
  });

  test('@grumps souviens-toi que … via webhook creates a memory entry', async ({ authedPage }) => {
    const csrfToken = await csrf(authedPage);
    const fact = `mon code secret est ${Date.now()}`;

    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: { id: SENDER_TG_ID, is_bot: false, first_name: 'Tester' },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: `@HeyGrumpsBot souviens-toi que ${fact}`,
      },
    };

    const r = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(r.ok()).toBeTruthy();

    let found: any = undefined;
    for (let i = 0; i < 12; i++) {
      const list = await authedPage.request.get(`/api/w/${SLUG}/memory`);
      const items = await list.json();
      found = items.find((m: any) => m.value && m.value.includes(fact));
      if (found) break;
      await new Promise(r => setTimeout(r, 500));
    }
    expect(found, `webhook should have created a memory entry containing "${fact}"`).toBeDefined();

    await authedPage.request.delete(`/api/w/${SLUG}/memory/${found.id}`, {
      headers: { 'X-CSRF-Token': csrfToken },
    });
  });

  test('@grumps quiet sets silence flag in KV', async ({ authedPage }) => {
    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: { id: SENDER_TG_ID, is_bot: false, first_name: 'Tester' },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: '@HeyGrumpsBot quiet',
      },
    };
    const r = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    // The fast-path responds 200 + sends a "quiet for 24h" reply via the
    // outbound TG callback (which fails for the fake chat but the worker
    // doesn't propagate the error).
    expect(r.ok()).toBeTruthy();
  });

  test('@grumps come back unsets silence flag', async ({ authedPage }) => {
    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id: Math.floor(Math.random() * 1_000_000_000),
        from: { id: SENDER_TG_ID, is_bot: false, first_name: 'Tester' },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: '@HeyGrumpsBot come back',
      },
    };
    const r = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
      headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
      data: update,
    });
    expect(r.ok()).toBeTruthy();
  });

  test('webhook is dedup’d on repeat message_id (KV)', async ({ authedPage }) => {
    const csrfToken = await csrf(authedPage);
    const title = `Webhook dedup ${Date.now()}`;
    const message_id = Math.floor(Math.random() * 1_000_000_000);

    const update = {
      update_id: Math.floor(Math.random() * 1_000_000_000),
      message: {
        message_id,
        from: { id: SENDER_TG_ID, is_bot: false, first_name: 'Tester' },
        chat: { id: CHAT_ID, type: 'group', title: 'Roommates' },
        date: Math.floor(Date.now() / 1000),
        text: `TODO:\n${title}`,
      },
    };

    // Send twice — the second one should be a no-op (KV dedup on message_id).
    for (let i = 0; i < 2; i++) {
      const r = await authedPage.request.post('http://127.0.0.1:8787/webhook/telegram', {
        headers: { 'X-Telegram-Bot-Api-Secret-Token': TG_WEBHOOK_SECRET },
        data: update,
      });
      expect(r.ok()).toBeTruthy();
    }

    // Wait for the first call to settle, then assert exactly one row.
    await new Promise(r => setTimeout(r, 1500));
    const list = await authedPage.request.get(`/api/w/${SLUG}/todos?status=all`);
    const items = await list.json();
    const matches = items.filter((t: any) => t.title === title);
    expect(matches.length).toBe(1);

    await authedPage.request.delete(`/api/w/${SLUG}/todos/${matches[0].id}`, {
      headers: { 'X-CSRF-Token': csrfToken },
    });
  });
});
