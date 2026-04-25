#!/usr/bin/env node
/**
 * Landing page i18n build.
 *
 * Reads:
 *   - ../index.html        (canonical English source)
 *   - strings.{lang}.json  (key -> translated string)
 * Writes:
 *   - ../dist/index.html        (English, with hreflang links + lang=en)
 *   - ../dist/{lang}/index.html (one per non-EN locale)
 *   - ../dist/_redirects        (Cloudflare Pages routing)
 *   - ../dist/workspace.html, ../dist/{lang}/workspace.html
 *   - ../dist/demo/             (SPA bundle, copied from crates/spa/dist/)
 *   - ../dist/{lang}/demo/index.html shims that redirect to /demo/?lang={code}
 *
 * Mechanics:
 *   - data-i18n="key"                      → element textContent  ← strings[key]
 *   - data-i18n-attr-{name}="key"          → element[name]        ← strings[key]
 *   - data-i18n-badge="..."                → CSS attr() (used by .price-card.featured::before)
 */

import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const SRC_HTML = path.join(ROOT, 'index.html');
const OUT_DIR = path.join(ROOT, 'dist');

const LOCALES = [
  { code: 'en',    name: 'English',           dir: 'ltr' },
  { code: 'es',    name: 'Español',           dir: 'ltr' },
  { code: 'pt-BR', name: 'Português (BR)',    dir: 'ltr' },
  { code: 'fr',    name: 'Français',          dir: 'ltr' },
  { code: 'de',    name: 'Deutsch',           dir: 'ltr' },
  { code: 'it',    name: 'Italiano',          dir: 'ltr' },
  { code: 'ru',    name: 'Русский',           dir: 'ltr' },
  { code: 'tr',    name: 'Türkçe',            dir: 'ltr' },
  { code: 'ar',    name: 'العربية',           dir: 'rtl' },
  { code: 'hi',    name: 'हिन्दी',              dir: 'ltr', font: 'Devanagari' },
  { code: 'zh-CN', name: '简体中文',           dir: 'ltr', font: 'SC' },
  { code: 'ja',    name: '日本語',             dir: 'ltr', font: 'JP' },
  { code: 'ko',    name: '한국어',             dir: 'ltr', font: 'KR' },
  { code: 'id',    name: 'Bahasa Indonesia',  dir: 'ltr' },
];

const NOTO_FONTS = {
  Devanagari: 'family=Noto+Sans+Devanagari:wght@400;500;600;700&family=Noto+Serif+Devanagari:wght@400;700;800',
  SC:         'family=Noto+Sans+SC:wght@400;500;600;700&family=Noto+Serif+SC:wght@400;700;900',
  JP:         'family=Noto+Sans+JP:wght@400;500;600;700&family=Noto+Serif+JP:wght@400;700;900',
  KR:         'family=Noto+Sans+KR:wght@400;500;600;700&family=Noto+Serif+KR:wght@400;700;900',
  Arabic:     'family=Noto+Sans+Arabic:wght@400;500;600;700&family=Noto+Serif+Arabic:wght@400;700;800',
};

const escHtml = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const escAttr = s => String(s).replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

function applyTextI18n(html, strings, missing) {
  return html.replace(
    /(<([a-zA-Z0-9-]+)([^>]*?)\sdata-i18n="([^"]+)"([^>]*)>)([\s\S]*?)(<\/\2>)/g,
    (full, openTag, tagName, beforeAttr, key, afterAttr, inner, closeTag) => {
      if (/<[a-zA-Z]/.test(inner)) return full;
      const v = strings[key];
      if (v == null) { missing.add(key); return full; }
      return openTag + escHtml(v) + closeTag;
    }
  );
}

function applyAttrI18n(html, strings, missing) {
  return html.replace(
    /<([a-zA-Z0-9-]+)([^>]*?)\sdata-i18n-attr-([a-zA-Z-]+)="([^"]+)"([^>]*)>/g,
    (full, tagName, before, attrName, key, after) => {
      const v = strings[key];
      if (v == null) { missing.add(key); return full; }
      const stripped = (before + ' ' + after).replace(new RegExp(`\\s${attrName}="[^"]*"`, 'g'), '');
      return `<${tagName}${stripped} ${attrName}="${escAttr(v)}">`;
    }
  );
}

// Resolve `data-i18n-alt-{slot}="key"` into baked `data-alt-{slot}-value="<localized>"`
// attributes. The browser-side JS (in index.html) reads these via
// `el.dataset.altMyValue` / `el.dataset.altStartValue`.
function applyAltI18n(html, strings, missing) {
  return html.replace(
    /\sdata-i18n-alt-([a-zA-Z]+)="([^"]+)"/g,
    (full, slot, key) => {
      const v = strings[key];
      if (v == null) { missing.add(key); return full; }
      const slotLower = slot.toLowerCase();
      // The original attribute is preserved AND a new data-alt-<slot>-value attribute is added.
      // The original is harmless (the JS doesn't read it); preserved for traceability.
      return `${full} data-alt-${slotLower}-value="${escAttr(v)}"`;
    }
  );
}

// Strip any hreflang links already embedded in the source (e.g. left over
// from a prior build that was committed) so the per-locale injected ones
// take effect.
function stripStaleHreflangs(html) {
  return html.replace(/<link rel="alternate" hreflang="[^"]*" href="[^"]*">\s*/g, '');
}

function injectHead(html, locale, hreflangLinks) {
  html = stripStaleHreflangs(html);
  const fontTag = locale.font
    ? `<link href="https://fonts.googleapis.com/css2?${NOTO_FONTS[locale.font]}&display=optional" rel="stylesheet">`
    : (locale.code === 'ar' ? `<link href="https://fonts.googleapis.com/css2?${NOTO_FONTS.Arabic}&display=optional" rel="stylesheet">` : '');

  const fontFallback = (locale.font || locale.code === 'ar') ? `
<style>
  body, .feature-card, .remember-card, .snippet, .chat-msg { font-family: var(--font-body), 'Noto Sans ${locale.font || 'Arabic'}', sans-serif; }
  h1, h2, h3, .nav-brand, .section-title, .step-num, .price-amount, .privacy-mark, .cal-mini-header, .mcd-fact { font-family: var(--font-display), 'Noto Serif ${locale.font || 'Arabic'}', serif; }
</style>` : '';

  html = html.replace(/<html[^>]*>/, `<html lang="${locale.code}" dir="${locale.dir}">`);
  return html.replace('</head>', `${fontTag}\n${fontFallback}\n${hreflangLinks}\n</head>`);
}

function buildHreflangs(canonicalBase) {
  const lines = LOCALES.map(l => {
    const slug = l.code === 'en' ? '' : `${l.code}/`;
    return `<link rel="alternate" hreflang="${l.code}" href="${canonicalBase}/${slug}">`;
  });
  lines.push(`<link rel="alternate" hreflang="x-default" href="${canonicalBase}/">`);
  return lines.join('\n');
}

// Rewrite the static lang switcher: prefix every `/code/` link with the
// site's sub-path (so links work on github.io/Grumps/, github.io/Grumps/fr/,
// etc.) and mark the current locale's link with class="current".
function applyLangSwitcher(html, code, name, sitePath) {
  const validCodes = new Set(LOCALES.map(l => l.code));
  // Strip any pre-existing class="current" on lang links.
  html = html.replace(
    /(<a href="(?:[^"]+)?\/[a-zA-Z-]+\/"[^>]*?)\sclass="current"/g,
    '$1'
  );
  html = html.replace(
    /<a href="(?:[^"]+)?\/([a-zA-Z-]+)\/"([^>]*)>/g,
    (full, slug, rest) => {
      if (!validCodes.has(slug)) return full;
      const cls = slug === code ? ' class="current"' : '';
      return `<a href="${sitePath}/${slug}/"${cls}${rest}>`;
    }
  );
  html = html.replace(/(<span id="current-lang">)[^<]*(<\/span>)/, `$1${escHtml(name)}$2`);
  return html;
}

async function loadStrings(code) {
  try {
    const raw = await fs.readFile(path.join(__dirname, `strings.${code}.json`), 'utf8');
    return JSON.parse(raw);
  } catch (e) {
    if (e.code === 'ENOENT') {
      console.warn(`  ! no strings file for ${code} — falling back to English`);
      return {};
    }
    throw e;
  }
}

async function main() {
  const masterRaw = await fs.readFile(SRC_HTML, 'utf8');
  await fs.mkdir(OUT_DIR, { recursive: true });

  // GitHub Pages reads `dist/CNAME` to bind the artifact to a custom domain.
  const cnamePath = path.join(__dirname, 'CNAME');
  const cnameExists = await fs.access(cnamePath).then(() => true).catch(() => false);
  if (cnameExists) await fs.copyFile(cnamePath, path.join(OUT_DIR, 'CNAME'));

  const canonicalBase = process.env.CANONICAL_BASE || 'https://grumps.app';
  const hreflangLinks = buildHreflangs(canonicalBase);

  // Sub-path the site is served under. Empty for a custom domain ("/"),
  // "/Grumps" for `username.github.io/Grumps/`. Controls the iframe src
  // and the per-locale demo shim redirect target — must match the
  // `--public-url` passed to `trunk build`.
  const sitePath = (process.env.SITE_PATH || '').replace(/\/$/, '');

  const spaDistExists = await fs.access(path.join(ROOT, 'crates', 'spa', 'dist', 'index.html'))
    .then(() => true).catch(() => false);

  for (const locale of LOCALES) {
    console.log(`> building ${locale.code} (${locale.name})`);
    let html = masterRaw;
    const missing = new Set();

    let strings = {};
    if (locale.code !== 'en') {
      strings = await loadStrings(locale.code);
      html = applyAttrI18n(html, strings, missing);
      html = applyTextI18n(html, strings, missing);
    } else {
      // Still need the alt resolutions for English.
      strings = await loadStrings('en').catch(() => ({}));
    }

    // Resolve hero CTA alt-i18n attributes for ALL locales (English included).
    html = applyAltI18n(html, strings, missing);

    if (missing.size > 0) {
      console.warn(`  ! ${missing.size} missing key(s): ${[...missing].slice(0, 6).join(', ')}${missing.size > 6 ? '…' : ''}`);
    }

    html = injectHead(html, locale, hreflangLinks);
    html = applyLangSwitcher(html, locale.code, locale.name, sitePath);

    // Iframe → live SPA in seed mode (when SPA dist is present). Always pass
    // ?lang= so the iframe locale matches the surrounding landing rather than
    // whatever the SPA happened to cache in localStorage from a prior visit.
    if (spaDistExists) {
      const iframeSrc = `${sitePath}/demo/?lang=${locale.code}`;
      html = html.replace(
        /<iframe\s+src="(workspace\.html|\/[^"]*demo[^"]*|\/workspace\.html)"/,
        `<iframe src="${iframeSrc}"`
      );
    }

    const outPath = locale.code === 'en'
      ? path.join(OUT_DIR, 'index.html')
      : path.join(OUT_DIR, locale.code, 'index.html');
    await fs.mkdir(path.dirname(outPath), { recursive: true });
    await fs.writeFile(outPath, html);
  }

  // Cloudflare Pages _redirects: lang aliases + SPA fallback for /demo/*.
  const redirects = [
    '# Lang aliases',
    ...LOCALES.filter(l => l.code !== 'en').map(l => `/${l.code} /${l.code}/ 301`),
    '/en /  301',
    '',
    '# SPA fallback for /demo/* — every deep URL resolves to the SPA shell',
    '/demo/* /demo/index.html 200',
    '',
    '# 404 fallback to English',
    '/* /index.html 200',
  ].join('\n') + '\n';
  await fs.writeFile(path.join(OUT_DIR, '_redirects'), redirects);

  // Demo mockup: copy English `workspace.html` to dist root + each
  // translated `workspace.{lang}.html` into its locale folder. Falls back
  // to English when a locale's translated mockup is missing.
  const enWorkspace = await fs.readFile(path.join(ROOT, 'workspace.html'), 'utf8').catch(() => null);
  if (enWorkspace) await fs.writeFile(path.join(OUT_DIR, 'workspace.html'), enWorkspace);
  for (const locale of LOCALES) {
    if (locale.code === 'en') continue;
    const src = path.join(ROOT, `workspace.${locale.code}.html`);
    let html;
    try { html = await fs.readFile(src, 'utf8'); }
    catch (e) {
      if (e.code !== 'ENOENT') throw e;
      console.warn(`  ! no workspace.${locale.code}.html — falling back to English mockup`);
      html = enWorkspace;
    }
    if (html) await fs.writeFile(path.join(OUT_DIR, locale.code, 'workspace.html'), html);
  }

  // SPA bundle (if built via `cd crates/spa && trunk build --release --public-url /demo/`).
  if (spaDistExists) {
    const spaDist = path.join(ROOT, 'crates', 'spa', 'dist');
    const demoOut = path.join(OUT_DIR, 'demo');
    await fs.mkdir(demoOut, { recursive: true });
    for (const entry of await fs.readdir(spaDist, { withFileTypes: true })) {
      const srcP = path.join(spaDist, entry.name);
      const dstP = path.join(demoOut, entry.name);
      if (entry.isDirectory()) await fs.cp(srcP, dstP, { recursive: true });
      else await fs.copyFile(srcP, dstP);
    }
    console.log(`  + SPA demo bundle copied to ${path.relative(ROOT, demoOut)}/`);

    // Per-locale shim: tiny redirect to {sitePath}/demo/?lang={code} (avoids duplicating the wasm bundle).
    for (const locale of LOCALES) {
      if (locale.code === 'en') continue;
      const shim = `<!doctype html><meta charset="utf-8"><title>Grumps demo</title>` +
                   `<meta http-equiv="refresh" content="0; url=${sitePath}/demo/?lang=${locale.code}">` +
                   `<link rel="canonical" href="${sitePath}/demo/">` +
                   `<p>Redirecting to demo…</p>`;
      const dir = path.join(OUT_DIR, locale.code, 'demo');
      await fs.mkdir(dir, { recursive: true });
      await fs.writeFile(path.join(dir, 'index.html'), shim);
    }
  } else {
    console.warn('  ! no crates/spa/dist/ — run `cd crates/spa && trunk build --release --public-url /demo/` to enable the live demo');
  }

  console.log(`\n✓ built ${LOCALES.length} locales into ${path.relative(ROOT, OUT_DIR)}/`);
}

main().catch(err => { console.error(err); process.exit(1); });
