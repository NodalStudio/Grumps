#!/usr/bin/env node
// Loads the workspace schema migrations + the 3 seed fixtures into the
// running D1 shim (scripts/d1-shim/server.mjs), one in-memory SQLite per
// test workspace. Run this after the shim is up and before `wrangler dev`
// serves its first request — a per-workspace query against an unmigrated
// shim database would otherwise 500.
//
// The database ids used here (`e2e-test-grp1` etc.) are shim-only logical
// keys, not real Cloudflare D1 uuids — the shim creates a fresh in-memory
// database for any id it hasn't seen yet. `scripts/seed-e2e-index.sql`
// points the local INDEX_DB's `workspaces_meta` rows at these same ids.
//
// Usage: node scripts/e2e-setup.mjs [--shim-url http://127.0.0.1:8788]

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const shimUrlArgIdx = process.argv.indexOf('--shim-url');
const SHIM_URL =
  (shimUrlArgIdx !== -1 && process.argv[shimUrlArgIdx + 1]) ||
  process.env.D1_SHIM_URL ||
  'http://127.0.0.1:8788';

export const WORKSPACES = [
  { id: 'e2e-test-grp1', seed: 'seed-test-grp1.sql' },
  { id: 'e2e-test-dm1', seed: 'seed-test-dm1.sql' },
  { id: 'e2e-test-arc1', seed: 'seed-test-arc1.sql' },
];

async function waitForShim(timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${SHIM_URL}/_shim/health`);
      if (res.ok) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`d1-shim did not become healthy within ${timeoutMs}ms at ${SHIM_URL}`);
}

async function execOn(databaseId, sql) {
  const res = await fetch(`${SHIM_URL}/_shim/exec/${databaseId}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ sql }),
  });
  const body = await res.json().catch(() => ({}));
  if (!res.ok || !body.success) {
    throw new Error(`shim exec failed for ${databaseId}: ${res.status} ${JSON.stringify(body)}`);
  }
}

async function main() {
  await waitForShim();

  const migrationsDir = path.join(ROOT, 'migrations', 'workspace');
  const migrationFiles = fs
    .readdirSync(migrationsDir)
    .filter((f) => f.endsWith('.sql'))
    .sort();

  for (const ws of WORKSPACES) {
    for (const file of migrationFiles) {
      const sql = fs.readFileSync(path.join(migrationsDir, file), 'utf8');
      await execOn(ws.id, sql);
    }
    const seedSql = fs.readFileSync(path.join(ROOT, 'scripts', ws.seed), 'utf8');
    await execOn(ws.id, seedSql);
    console.log(`[e2e-setup] ${ws.id}: applied ${migrationFiles.length} migrations + seed`);
  }

  console.log('[e2e-setup] done');
}

main().catch((e) => {
  console.error('[e2e-setup] FAILED:', e);
  process.exit(1);
});
