// Single source of truth for the API origin in e2e fixtures.
//
// Mirrors `e2e_base_url()` in `backend-rs/tests/api/helpers.rs` for the API
// axis: `E2E_BASE_URL` pins an already-running API (CI: the docker-e2e job's
// API container on :3001; `playwright.config.ts` then omits its API webServer).
// When unset, fixtures hit the local default — backend-rs on :3001 — which is
// what `playwright.config.ts` starts for a zero-env local run.
//
// Fixtures are non-browser clients: they send no `Origin`/`Sec-Fetch-*`
// headers, so the API's CSRF layer treats them as same-origin. Do NOT add an
// `Origin` header here — the T56 fail-closed layer would 403 the request.
export const E2E_API: string =
  (process.env.E2E_BASE_URL ?? '').trim() || 'http://localhost:3001';
