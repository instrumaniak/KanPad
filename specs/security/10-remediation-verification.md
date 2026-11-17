# 10 — Remediation Verification

**Objective:** Ensure fixes actually work, don't regress, and residual risks are
explicitly accepted.

**Scope:** all SEC-* tasks; CI regression tests; final report.

---

## 1. Test plan

### 1.1 Per-finding verification
- For each finding in `findings-register.md`, after the fix:
  - Re-run the specific test that failed.
  - Re-run the phase's full test plan.
  - Mark status `Verified` with evidence (request/response or test output).

### 1.2 Regression tests in CI
- (Cross-reference, amended 2026-11-16: the Playwright-in-CI wiring is owned by
  `T017` in the arch-hardening plan — SEC-024 must extend, not duplicate, that
  job.)
- Cross-user 404 for each resource (03).
- CSRF 403 for cross-site (05).
- Throttle 429 on auth (05).
- Security header presence on 200/401/403/404/429/500 (05, 07).
- Cookie attribute matrix per env (02).
- Validation 400 for bad pagination/params (04).
- SQLi/XSS payload rejection (04).
- MCP/API-key surface (11, amended 2026-11-16): `Authorization: Bearer` rejected
  on `POST /api/v1/api-keys` (cookie-only minting) and on `/auth/*`; scope-denied
  tool call returns `isError`; revoked/expired key fails on the next request.

### 1.3 Re-scan
- Re-run section 1 recon commands from `01-reconnaissance-findings.md`.
- Re-run `cargo audit`, `cargo deny`, `npm audit`.
- Re-run secret scan.

### 1.4 Final report
- Executive summary (business risk, top 5).
- Findings register final state.
- Residual risk acceptance doc for anything not fixed (with owner + rationale).

## 2. Exit criteria (audit complete)
- Zero Critical/High findings open.
- All regression tests in CI and green.
- Live config matches hardening checklist (verified).
- Final report delivered.

## 3. Task mapping
- `SEC-024` add security regression tests to CI.
- `SEC-025` final re-scan + report.
- `SEC-028` MCP/API-key surface audit (11) — see the phase file; its
  regression coverage rides SEC-024's job.