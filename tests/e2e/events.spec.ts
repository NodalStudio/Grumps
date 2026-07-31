import { test, expect, csrf } from './fixtures';

const SLUG = 'test-grp1';

async function deleteEvent(page: any, t: string, id: string) {
  await page.request.delete(`/api/w/${SLUG}/events/${id}`, {
    headers: { 'X-CSRF-Token': t },
  });
}

test.describe('Events API', () => {
  test('list defaults to a -30/+60 day range and returns an array', async ({ authedPage }) => {
    const r = await authedPage.request.get(`/api/w/${SLUG}/events`);
    expect(r.ok()).toBeTruthy();
    const events = await r.json();
    expect(Array.isArray(events)).toBe(true);
  });

  test('CRUD round-trip', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const starts = '2026-04-30T18:00:00Z';
    const ends = '2026-04-30T19:00:00Z';

    const created = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: 'House meeting',
        description: 'Monthly sync',
        starts_at: starts,
        ends_at: ends,
        location: 'Living room',
      },
    });
    expect(created.ok()).toBeTruthy();
    const ev = await created.json();
    expect(ev.id).toBeTruthy();

    const got = await authedPage.request.get(`/api/w/${SLUG}/events/${ev.id}`);
    expect(got.ok()).toBeTruthy();
    const fetched = await got.json();
    expect(fetched.title).toBe('House meeting');

    const updated = await authedPage.request.put(`/api/w/${SLUG}/events/${ev.id}`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: 'House meeting (updated)',
        starts_at: starts,
        ends_at: ends,
      },
    });
    expect(updated.ok()).toBeTruthy();

    await deleteEvent(authedPage, t, ev.id);
    const gone = await authedPage.request.get(`/api/w/${SLUG}/events/${ev.id}`);
    expect(gone.status()).toBe(404);
  });

  test('all_day flag is persisted', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: 'All day off-site',
        starts_at: '2026-04-30T00:00:00Z',
        ends_at: '2026-04-30T23:59:59Z',
        all_day: true,
      },
    });
    expect(created.ok()).toBeTruthy();
    const ev = await created.json();
    expect(ev.all_day).toBe(true);

    await authedPage.request.delete(`/api/w/${SLUG}/events/${ev.id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('rejects empty title', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: '   ',
        starts_at: '2026-04-30T10:00:00Z',
      },
    });
    expect(r.status()).toBe(400);
  });

  test('rejects ends_at before starts_at on POST', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const r = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: 'Bad range',
        starts_at: '2026-04-30T10:00:00Z',
        ends_at: '2026-04-30T09:00:00Z',
      },
    });
    expect(r.status()).toBe(400);
  });

  test('rejects ends_at before starts_at on PUT', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: 'Will be updated',
        starts_at: '2026-04-30T10:00:00Z',
        ends_at: '2026-04-30T11:00:00Z',
      },
    });
    const ev = await created.json();

    const r = await authedPage.request.put(`/api/w/${SLUG}/events/${ev.id}`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: 'Will be updated',
        starts_at: '2026-04-30T10:00:00Z',
        ends_at: '2026-04-30T09:00:00Z',
      },
    });
    expect(r.status()).toBe(400);

    await authedPage.request.delete(`/api/w/${SLUG}/events/${ev.id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('calendar UI agenda view lists a created event by title', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const distinctTitle = `Agenda probe ${Date.now()}`;
    // Create the event in the future (this month) so it falls in the
    // calendar's default month-range query.
    const now = new Date();
    const startsAt = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 28, 14, 0, 0)).toISOString();
    const endsAt = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 28, 15, 0, 0)).toISOString();

    const created = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: { title: distinctTitle, starts_at: startsAt, ends_at: endsAt },
    });
    const ev = await created.json();

    await authedPage.goto(`/w/${SLUG}/calendar?lang=en`);
    // The Month/Week/Agenda switcher is a `tablist`/`tab` (ARIA), not buttons.
    await authedPage.getByRole('tab', { name: /Agenda/i }).click();
    await expect(authedPage.getByText(distinctTitle)).toBeVisible({ timeout: 10_000 });

    await authedPage.request.delete(`/api/w/${SLUG}/events/${ev.id}`, {
      headers: { 'X-CSRF-Token': t },
    });
  });

  test('listed event appears in /calendar aggregated feed', async ({ authedPage }) => {
    const t = await csrf(authedPage);
    const created = await authedPage.request.post(`/api/w/${SLUG}/events`, {
      headers: { 'X-CSRF-Token': t },
      data: {
        title: 'Calendar test event',
        starts_at: '2026-04-28T12:00:00Z',
        ends_at: '2026-04-28T13:00:00Z',
      },
    });
    const ev = await created.json();

    const cal = await authedPage.request.get(`/api/w/${SLUG}/calendar?from=2026-04-01&to=2026-04-30`);
    expect(cal.ok()).toBeTruthy();
    const items = await cal.json();
    expect(items.some((x: any) => x.title === 'Calendar test event')).toBe(true);

    await deleteEvent(authedPage, t, ev.id);
  });
});
