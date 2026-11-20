# Load testing (k6)

## Safety guards (READ FIRST)

- Default `BASE_URL=http://localhost:5173` (the only published compose port).
- Refuse non-loopback hosts without `K6_ALLOW_REMOTE=1` (enforced by k6 usage and by `seed.mjs` BASE_URL guard).
- Never paste origins from `docs/deployment.md` (they are real prod URLs).
- Seed script uses explicit env vars only — NO `dotenv/config` autoload (repo-root
  `.env` holds live prod secrets). Target DB name must match `kanpad_(e2e|load|test)` when provided.
- Throttle raise via `compose.load.yaml` override only — never edit `compose.yaml`
  or repo-root `.env`.
- Compose runs production → session cookies are `Secure`; Go cookiejar drops them
  over plain HTTP → send `Cookie` explicitly as a request header.
- 100+ VU x Argon2 is CPU-bound by design → expect high CPU; don't crash the machine.
- Argon2 login cost is measured, never "fixed" (HASH_COST floor is a security rule).
- `/metrics` smoke: scrape in-process or loopback only; never add `METRICS_PORT`.

## Run commands

Seed (throwaway target):
```
BASE_URL=http://localhost:5173 DB_NAME=kanpad_load DB_HOST=127.0.0.1 node scripts/load/seed.mjs
```

k6 (loopback only):
```
BASE_URL=http://localhost:5173 COOKIE='session=...' k6 run scripts/load/board-smoke.js
```

Compose with throttle raise:
```
docker compose -f compose.yaml -f scripts/load/compose.load.yaml up
```

Remote guard:
```
K6_ALLOW_REMOTE=1 BASE_URL=https://... k6 run scripts/load/board-smoke.js
```
