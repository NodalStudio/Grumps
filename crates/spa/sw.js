// CACHE_NAME suffix is stamped by the Trunk post-build hook (see
// crates/spa/Trunk.toml) with the current git SHA, so every deploy gets
// a fresh cache and the `activate` handler below evicts the previous one.
const CACHE_NAME = 'grumps-__BUILD_HASH__';

// Hashed bundles emitted by Trunk look like `grumps-spa-<16hex>.js`,
// `tailwind.out-<16hex>.css`, etc. Their content is immutable for a
// given hash, so cache-first is safe and fast.
const HASHED_ASSET_RE = /-[0-9a-f]{16}\.(?:js|wasm|css)$/;

self.addEventListener('install', () => {
  // Skip the default "wait for all clients to close" — we want the new
  // SW to take over the moment a deploy lands, paired with network-first
  // HTML below so open tabs pick up the new shell on next navigation.
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (event) => {
  const req = event.request;

  // Only handle GET; POST/PUT/PATCH/DELETE go straight to the network.
  if (req.method !== 'GET') return;

  const url = new URL(req.url);

  // API + auth: never cached.
  if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/auth/')) {
    return;  // let the browser handle it normally
  }

  // Cross-origin: don't intercept (fonts.googleapis.com, etc.)
  if (url.origin !== self.location.origin) return;

  // Hashed bundles: cache-first, immutable. The filename changes when
  // the content changes, so a cache hit is always correct.
  if (HASHED_ASSET_RE.test(url.pathname)) {
    event.respondWith(
      caches.match(req).then((cached) => {
        if (cached) return cached;
        return fetch(req).then((res) => {
          if (res.ok) {
            const clone = res.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(req, clone));
          }
          return res;
        });
      })
    );
    return;
  }

  // Everything else (HTML shell, manifest, favicon, route URLs): the SPA
  // shell IS the entry, so we want the freshly deployed version. Use
  // network-first; fall back to cache only when offline so the app keeps
  // working without connectivity.
  event.respondWith(
    fetch(req).then((res) => {
      if (res.ok) {
        const clone = res.clone();
        caches.open(CACHE_NAME).then((cache) => cache.put(req, clone));
      }
      return res;
    }).catch(() => caches.match(req).then((cached) => cached || caches.match('/index.html')))
  );
});
