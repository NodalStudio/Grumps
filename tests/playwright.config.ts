import { defineConfig, devices } from '@playwright/test';

/**
 * Grumps E2E test config.
 *
 * Assumes the SPA + worker are already running locally:
 *   - wrangler dev --port 8787 --var ENVIRONMENT:development
 *   - trunk serve --release --port 8080  (with Trunk.toml proxy_backend)
 *
 * Drive via `npm run test` from this `tests/` dir. Auth shortcut: tests
 * call `/auth/telegram/verify` with `dev_bypass: true` (gated server-side
 * by the GRUMPS_DEV_AUTH_BYPASS secret + ENVIRONMENT != "production").
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: false, // Tests share state (the seeded local D1)
  workers: 1,
  reporter: [['list'], ['html', { open: 'never' }]],

  use: {
    baseURL: 'http://localhost:8080',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
