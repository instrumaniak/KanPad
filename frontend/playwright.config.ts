import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig, devices } from '@playwright/test';

// KanPad e2e — prod-like two-origin topology (T017 / arch-hardening R7).
//
//   SPA (Vite, :5173)  --absolute VITE_API_URL-->  API (backend-rs, :3001)
//
// The browser path is exactly what T007 hardened: cross-origin cookies
// (SameSite=Lax), CORS allowlist (`CORS_ORIGINS` default = http://localhost:5173)
// and the T56 cross-origin mutating-request check. Do NOT point the client at
// the Vite dev proxy (same-origin shortcut — defeats the point of the suite).
//
// Local one-command path (provisions `kanpad_e2e` + migrations, then runs this):
//   npm run test:e2e
//
// Pin semantics (test env overrides; CI exports E2E_BASE_URL):
//   E2E_BASE_URL=<api-origin>  — API webServer omitted; fixtures + browser use
//                                the pin (mirrors backend-rs helpers' e2e_base_url()).
//   E2E_SPA_URL=<spa-origin>   — SPA webServer omitted; `baseURL` = the pin.
//   (neither set)              — both servers are started; works with no env set
//                                on a machine with backend-rs/.env (see below).
//
// API webServer env (always): APP_ENV=test (non-prod: cookies not Secure),
// THROTTLE_*=10000 (effectively unbounded — every request comes from
// 127.0.0.1 and the default auth bucket is 5/min, which would 429 mid-suite.
// NOTE: config.rs/.env.example document THROTTLE_*=0 as "disable", but
// api_router()'s build_layer() coerces burst_size to .max(1), making 0 mean
// 1 req/min, not "off" — see spec T017 Notes 2026-11-18. Use 10000, not 0.),
// DATABASE_URL=...kanpad_e2e (throwaway
// schema; never the dev DB). reuseExistingServer is false for the API on
// purpose — a stray server cannot guarantee kanpad_e2e + test env. The SPA
// reuses an already-running Vite dev server outside CI (caveat: it must be
// started with VITE_API_URL=http://localhost:3001, otherwise the browser path
// silently degrades to same-origin).
//
// Port clash rule: never run while something already holds :3001 (or :5173,
// without CI's reuse rules). Pin with E2E_BASE_URL / E2E_SPA_URL instead. The
// Rust suites bind ephemeral ports and are never affected.

const here = dirname(fileURLToPath(import.meta.url));
const apiOrigin = process.env.E2E_BASE_URL ?? 'http://localhost:3001';
const spaOrigin = process.env.E2E_SPA_URL ?? 'http://localhost:5173';

/** Parse a dotenv file (KEY=value, optional quotes) — no dependency needed. */
function parseEnvFile(path: string): Record<string, string> {
  try {
    const out: Record<string, string> = {};
    for (const line of readFileSync(path, 'utf8').split('\n')) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('#')) continue;
      const eq = trimmed.indexOf('=');
      if (eq <= 0) continue;
      const key = trimmed.slice(0, eq).trim();
      let value = trimmed.slice(eq + 1).trim();
      if (
        (value.startsWith('"') && value.endsWith('"') && value.length >= 2) ||
        (value.startsWith("'") && value.endsWith("'") && value.length >= 2)
      ) {
        value = value.slice(1, -1);
      }
      out[key] = value;
    }
    return out;
  } catch {
    return {};
  }
}

/**
 * DATABASE_URL for the API webServer: E2E_DATABASE_URL wins; otherwise derive
 * from ../backend-rs/.env (gitignored, local-only) — DB_* host/port/user/
 * password, with the *name forced to* kanpad_e2e and userinfo percent-encoded.
 * Never logged; only handed to the spawned server process.
 */
function e2eDatabaseUrl(): string {
  if (process.env.E2E_DATABASE_URL) return process.env.E2E_DATABASE_URL;
  const env = parseEnvFile(join(here, '..', 'backend-rs', '.env'));
  const host = env.DB_HOST ?? 'localhost';
  const port = env.DB_PORT ?? '3306';
  const user = env.DB_USERNAME;
  const password = env.DB_PASSWORD;
  if (!user || password === undefined) {
    throw new Error(
      'T017 e2e: cannot derive DATABASE_URL — set E2E_DATABASE_URL or provide ' +
        'DB_USERNAME/DB_PASSWORD in backend-rs/.env (or run `npm run test:e2e`).'
    );
  }
  const userinfo = `${encodeURIComponent(user)}:${encodeURIComponent(password)}`;
  return `mysql://${userinfo}@${host}:${port}/kanpad_e2e?charset=utf8mb4&collation=utf8mb4_unicode_ci`;
}

const webServer: Array<Record<string, unknown>> = [];
if (!process.env.E2E_SPA_URL) {
  webServer.push({
    command: 'npm run dev',
    url: spaOrigin,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    // Absolute API origin => the two-origin path, never the dev proxy.
    env: { VITE_API_URL: apiOrigin },
  });
}
if (!process.env.E2E_BASE_URL) {
  webServer.push({
    command: 'cargo run --locked -- serve',
    cwd: join(here, '..', 'backend-rs'),
    url: `${apiOrigin}/health`,
    // Always boot our own against kanpad_e2e + test env.
    reuseExistingServer: false,
    timeout: 300_000,
    env: {
      APP_ENV: 'test',
      PORT: '3001',
      SESSION_SECRET: 'e2e-session-secret-not-for-production-0123456789abcdef',
      DATABASE_URL: e2eDatabaseUrl(),
      THROTTLE_AUTH_PER_MIN: '10000',
      THROTTLE_GLOBAL_PER_MIN: '10000',
    },
  });
}

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 4,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: spaOrigin,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: webServer.length ? webServer : undefined,
});
