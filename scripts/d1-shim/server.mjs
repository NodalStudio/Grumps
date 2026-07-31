#!/usr/bin/env node
// Minimal local stand-in for the Cloudflare D1 HTTP API, backed by
// better-sqlite3 (real SQLite, FTS5 included — `migrations/workspace/
// 0002_memory.sql`'s memory search needs it, and Node's own built-in
// `node:sqlite` doesn't bundle it).
//
// `D1RestClient::query` (crates/worker/src/d1_rest.rs) hardcodes
// `https://api.cloudflare.com` unless `D1_REST_BASE` is set, in which case
// every request goes here instead — same path shape, same request/response
// JSON contract (`D1Query` / `D1Response`), so the worker can't tell the
// difference. This unblocks the per-workspace e2e specs (todos, notes,
// members, memory, events, scheduled, ...) without any live Cloudflare
// account access. See issue #9 for the fuller design this is a pragmatic
// slice of.
//
// Each `database_id` gets its own in-memory SQLite instance, created lazily
// on first use and kept for the lifetime of this process — exactly what a
// single e2e run needs (no persistence across runs, no cross-test-file
// bleed beyond what the seed data already assumes).
//
// Two admin-only routes (never hit by the worker) let the e2e setup step
// load schema + fixtures before Playwright starts:
//   POST /_shim/exec/:id   { sql }   run a raw (possibly multi-statement,
//                                    including CREATE TRIGGER ... BEGIN...END)
//                                    batch against database :id
//   GET  /_shim/health               liveness probe for the CI wait-loop

import http from 'node:http';
import Database from 'better-sqlite3';

const PORT = process.env.D1_SHIM_PORT ? Number(process.env.D1_SHIM_PORT) : 8788;

/** @type {Map<string, import('better-sqlite3').Database>} */
const dbs = new Map();

// Indirected through a helper (rather than `db.exec(sql)` inline at every
// call site) purely so there's one place that documents *why* this batch
// path exists: better-sqlite3's `exec` runs a raw, possibly multi-statement
// SQL string through SQLite's own parser — the only way to apply a
// migration file containing a `CREATE TRIGGER ... BEGIN ... END;` body
// without hand-splitting it (SQLite must see the whole trigger at once).
function runBatch(db, sql) {
  const method = db['exec'];
  return method.call(db, sql);
}

function getDb(id) {
  let db = dbs.get(id);
  if (!db) {
    db = new Database(':memory:');
    db.pragma('journal_mode = MEMORY');
    dbs.set(id, db);
  }
  return db;
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let data = '';
    req.on('data', (c) => (data += c));
    req.on('end', () => resolve(data));
    req.on('error', reject);
  });
}

function sendJson(res, status, body) {
  res.writeHead(status, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(body));
}

function isSelectish(sql) {
  return /^\s*(select|pragma|with)\b/i.test(sql);
}

/// Split multi-statement SQL on top-level `;` while keeping trigger bodies
/// (`BEGIN ... END;`) intact — mirrors `split_sql_statements` in
/// `crates/worker/src/d1_rest.rs`. Used only to decide "one statement or a
/// batch", not to execute (a real multi-statement batch is handed to
/// `runBatch` as-is, which parses statement boundaries itself, so this
/// never needs to be byte-perfect for execution — only for counting).
function splitStatements(sql) {
  const out = [];
  let cur = '';
  let depth = 0;
  let i = 0;
  const isWordChar = (c) => /[A-Za-z0-9_]/.test(c ?? '');
  const matchesWordCi = (s, i, word) => {
    const prev = s[i - 1];
    if (isWordChar(prev)) return false;
    if (s.slice(i, i + word.length).toUpperCase() !== word) return false;
    const next = s[i + word.length];
    if (isWordChar(next)) return false;
    return true;
  };
  while (i < sql.length) {
    if (matchesWordCi(sql, i, 'BEGIN')) {
      depth += 1;
      cur += sql.slice(i, i + 5);
      i += 5;
      continue;
    }
    if (depth > 0 && matchesWordCi(sql, i, 'END')) {
      depth -= 1;
      cur += sql.slice(i, i + 3);
      i += 3;
      continue;
    }
    const c = sql[i];
    if (c === ';' && depth === 0) {
      const trimmed = cur.trim();
      if (trimmed) out.push(trimmed);
      cur = '';
      i += 1;
      continue;
    }
    cur += c;
    i += 1;
  }
  const trimmed = cur.trim();
  if (trimmed) out.push(trimmed);
  return out;
}

// D1's REST params array carries `?1,?2,...`-style values, and this
// codebase sometimes reuses the same index twice in one statement (e.g.
// `upsert_member`'s `ON CONFLICT ... SET display_name = ?3` reusing the
// `?3` already bound in the INSERT VALUES list — SQLite explicitly supports
// binding the same numbered placeholder more than once). A naive `?N -> ?`
// text rewrite plus positional-array binding breaks exactly that case: it
// turns two `?3` occurrences into two anonymous `?`s needing two separate
// values instead of one shared one. better-sqlite3 also doesn't bind `?N`
// via a plain array at all ("too many parameter values") — it wants an
// object keyed by the numeric string, e.g. `{1: v1, 2: v2}` — which is
// exactly what's needed here too: build that object straight from the
// params array with no SQL text rewriting.
function numberedParams(params) {
  const obj = {};
  params.forEach((v, i) => {
    obj[i + 1] = v;
  });
  return obj;
}

function runSingleStatement(db, sql, params) {
  const stmt = db.prepare(sql);
  const bound = numberedParams(params);
  if (isSelectish(sql)) {
    const rows = stmt.all(bound);
    return { results: rows, meta: { changes: 0, last_row_id: 0 } };
  }
  const info = stmt.run(bound);
  return {
    results: [],
    meta: { changes: info.changes, last_row_id: Number(info.lastInsertRowid) },
  };
}

function runQuery(databaseId, sql, params) {
  const db = getDb(databaseId);
  const statements = splitStatements(sql);
  if (statements.length <= 1) {
    return runSingleStatement(db, sql, params ?? []);
  }
  // Batch (migrations / seed scripts via `exec_statements`): never carries
  // params, and needs SQLite's own statement-boundary parsing (trigger
  // bodies), so hand the original text to the batch path rather than the
  // split copy.
  runBatch(db, sql);
  return { results: [], meta: { changes: 0, last_row_id: 0 } };
}

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, `http://127.0.0.1:${PORT}`);

    if (req.method === 'GET' && url.pathname === '/_shim/health') {
      sendJson(res, 200, { ok: true, databases: dbs.size });
      return;
    }

    const setupMatch = url.pathname.match(/^\/_shim\/exec\/([^/]+)$/);
    if (setupMatch && req.method === 'POST') {
      const { sql } = JSON.parse(await readBody(req));
      runBatch(getDb(setupMatch[1]), sql);
      sendJson(res, 200, { success: true });
      return;
    }

    // Mirrors the real D1 REST create-database call (`D1RestClient::create_database`)
    // used by provisioning.rs when the worker onboards a brand new workspace
    // during an e2e run (e.g. a "first message in a new group" webhook test).
    const createDbMatch = url.pathname.match(/^\/client\/v4\/accounts\/[^/]+\/d1\/database$/);
    if (createDbMatch && req.method === 'POST') {
      const uuid =
        'shim-' + Math.random().toString(36).slice(2) + Date.now().toString(36);
      getDb(uuid); // provision eagerly so the first real query never 404s
      sendJson(res, 200, { result: { uuid }, success: true, errors: [] });
      return;
    }

    const queryMatch = url.pathname.match(
      /^\/client\/v4\/accounts\/[^/]+\/d1\/database\/([^/]+)\/query$/
    );
    if (queryMatch && req.method === 'POST') {
      const databaseId = queryMatch[1];
      const { sql, params } = JSON.parse((await readBody(req)) || '{}');
      try {
        const { results, meta } = runQuery(databaseId, sql, params);
        sendJson(res, 200, {
          result: [{ results, meta }],
          success: true,
          errors: [],
        });
      } catch (e) {
        sendJson(res, 200, {
          result: [],
          success: false,
          errors: [{ message: e instanceof Error ? e.message : String(e) }],
        });
      }
      return;
    }

    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('not found');
  } catch (e) {
    res.writeHead(500, { 'Content-Type': 'text/plain' });
    res.end(e instanceof Error ? (e.stack ?? e.message) : String(e));
  }
});

server.listen(PORT, () => {
  console.log(`[d1-shim] listening on http://127.0.0.1:${PORT}`);
});
