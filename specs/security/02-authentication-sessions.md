# 02 — Authentication & Session Management

**Objective:** Verify credentials, password hashing, session lifecycle, fixation
defense, brute-force protection, and account enumeration resistance.

**Scope:** `backend-rs/src/features/auth/*`, session store (`common/session_store.rs`),
config (SESSION_SECRET, HASH_COST, REGISTRATION_ENABLED), frontend auth flows.

**Threat actors:** anonymous attacker, attacker with a valid low-privilege account.

---

## 1. Test plan

### 1.1 Password policy
- Register with password < 8 chars → 400 with errors[].
- Register with no digit/symbol → 400.
- Register with password > 72 bytes (multibyte) → 400 (byte cap, not char cap).
- Login with a weak legacy password (if any imported) → must reach 401, not 400
  (login = non-empty + byte cap only, never strength).
- Mass-assignment: send `role`, `user_id`, `id` in register body → ignored.

### 1.2 Hashing
- Stored PHC matches `$argon2id$v=19$m=19456,t=HASH_COST,p=1$`.
- Legacy bcrypt rows (`$2a$`/`$2b$` cost 10) verify + rehash to Argon2id in same
  request; row updated.
- Unknown email path runs dummy Argon2 verify (timing — statistical check).
- `HASH_COST` prod floor: boot fails when `APP_ENV=production && HASH_COST<2`.

### 1.3 Session fixation
- `session.cycle_id()` on register, login, and privilege change → cookie value
  changes (compare Set-Cookie before/after).
- Pre-login session ID is not reused post-login.

### 1.4 Session expiry & revocation
- Sliding 24h TTL; expired row sweeper (60s) removes rows.
- Old `connect.sid` cookie (pre-cutover, Nest codec) → 401 (force re-login).
- Rotating `SESSION_SECRET` invalidates all sessions.
- `GET /api/v1/auth/me` re-looks up user row: deleted → 401, demoted → 403.

### 1.5 Brute force
- Auth throttle 5 req/min/IP → 429 + `Retry-After` on 6th.
- Verify throttle keys on real client IP behind proxy (TRUSTED_PROXIES).
- Global throttle 120/min/IP does not block normal frontend bursts.

### 1.6 Account enumeration
- Login unknown email vs wrong password: identical status (401), body, and timing
  (within tolerance).
- `/auth/me`, register duplicate email: no user-existence leak (register returns
  409 conflict generically).

### 1.7 Registration toggle
- `REGISTRATION_ENABLED=false` → register returns error, no user created.
- `create-admin`: single-admin guard; `--force` resets; exit codes 0/1/2/3.

### 1.8 Cookie attributes (live + per env)
- Prod `Set-Cookie`: starts with `__Host-kanpad.sid=`, carries `Secure`, `Path=/`,
  `HttpOnly`, `SameSite=Lax`, NO `Domain`.
- Dev/test: starts with `kanpad.sid=`, no `Secure`.
- No environment emits `connect.sid`.

## 2. Live findings so far
- Login unknown-email → 401 generic body (no enumeration via message). ✓
- No Set-Cookie on failed login. ✓
- CSRF cross-site → 403. ✓
- (Cookie attribute matrix not yet captured on a successful login — need a test
  account. Create one during the audit or use a throwaway registration if enabled.)

## 3. Remediation guidance
- If `SESSION_SECRET` < 32 bytes on prod: regenerate with `openssl rand -hex 32`
  (64 chars = 32 bytes) or longer, restart, accept session wipe.
- If throttle keys on proxy IP: set `TRUSTED_PROXIES` to the proxy's CIDR and
  ensure proxy overwrites XFF.
- If timing leak on unknown email: keep dummy Argon2 verify; consider equalizing
  response time with a minimum delay (only if measurement shows leak).

## 4. Exit criteria
- All 1.1–1.8 checks pass on local compose stack + live.
- Cookie attribute matrix test added to CI (T11-style).
- Timing test recorded (pass or documented residual).

## 5. Task mapping
- `SEC-001` SESSION_SECRET strength verification/rotation.
- `SEC-002` throttle IP extraction behind proxy.
- `SEC-003` cookie attribute regression test.