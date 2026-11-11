# 09 — Logging, Monitoring & Incident Response

**Objective:** Ensure no PII/secrets in logs, adequate audit trail and alerting,
and a tested incident-response capability.

**Scope:** tracing config, `#[tracing::instrument]` usage, metrics, DB probe
transitions, IR runbook.

---

## 1. Test plan

### 1.1 PII in logs
- Grep gate already: no `instrument(skip(state))` (must skip session + body).
  Re-verify after any new handler.
- Capture logs during login (success + fail) and register; search for email,
  password, token, cookie, session id. Must be absent.
- T05: bad DB password at boot → no password substring in logs.
- Error paths: 500 logs full sqlx::Error server-side (OK, not to client) but
  must not include secrets.

### 1.2 Audit trail
- Currently no activity feed (out of scope per spec). Minimum: log auth events
  (login success/fail with IP + user id, register, logout, password change,
  admin actions) as structured events.
- Retention: define (e.g. 90 days) and storage.

### 1.3 Alerting
- 5xx rate spike, 401/403 spike, throttle rejections (429 count), DB probe
  transitions (Up→Down = error), pool saturation (idle=0).
- `/metrics` scrape must not be public (separate listener, firewall).

### 1.4 Incident response
- Runbook: compromised SESSION_SECRET → rotate + restart (all sessions die).
- Runbook: DB restore from backup; verify restore.
- Runbook: disable registration (`REGISTRATION_ENABLED=false`) + restart.
- Runbook: block an IP (proxy/firewall level, since no in-app IP block).
- Contact/escalation list (solo owner → self).

### 1.5 Backups
- Automated DB backups; encrypted at rest; tested restore (quarterly).
- Backup must not contain plaintext secrets in `.env` (separate).

## 2. Live findings so far
- Metrics port status unknown (METRICS_PORT likely unset → disabled, good).
  Verify on server.
- No audit trail currently (spec confirms no activity entity).

## 3. Remediation guidance
- Add structured auth-event logging (tracing events, not spans).
- Document IR runbook in `docs/` or `specs/security/`.
- Test restore from backup.

## 4. Exit criteria
- No PII/secrets in logs (verified by grep + manual capture).
- IR runbook exists and is tested (restore drill).
- Alerting thresholds documented.

## 5. Task mapping
- `SEC-021` auth-event audit logging.
- `SEC-022` IR runbook + backup restore drill.
- `SEC-023` metrics/alerting documentation.