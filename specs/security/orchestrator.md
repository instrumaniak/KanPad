# Security Audit — Orchestrator / Progress Tracker

**Last updated:** 2026-11-10
**Mode:** local-first (no live scanning — provider blocks IPs)
**Owner:** project owner (solo)

## Current status
- Audit plan complete: specs/security/00..10 + quick-wins + findings-register.
- Live recon complete (passive + light active): headers, CORS, CSRF, TLS, port
  scan, gitleaks, ffuf. Provider blocked our IP after bursts -> no more live
  scanning. Remaining work is white-box + local compose (compose.yaml).
- White-box code review complete: findings F-03..F-06 added (see 01 S2.6).
- F-01 (MySQL exposed) mitigated (remote access removed) - verify closed.
- No implementation done. Quick wins QW-1..QW-10, QW-12..QW-15 pending
  (QW-0 done/mitigated, QW-11 done — gitleaks clean, see 01 §2.5).
- Downloaded tools in /tmp/opencode/tools (~52 MB) - removable.

## Completed
- [x] 00-overview.md - scope, methodology, severity, tooling
- [x] 01-reconnaissance-findings.md (+ tool results + code review S2.6)
- [x] 02..10 plan files
- [x] quick-wins.md (QW-0..QW-15)
- [x] findings-register.md (R/T/F rows)
- [x] Live recon (headers, CORS, CSRF, TLS, ports, gitleaks, ffuf)
- [x] White-box code review (backend-rs + frontend) — code review only; test execution pending

## In progress
- None (awaiting Build mode to start fixes)

## Next steps (ordered)
1. Verify MySQL closed: `nmap -Pn -p 3306 <host>` (single command, no burst).
2. Quick wins: QW-1 (SESSION_SECRET), QW-13/F-03 (CSRF instance), QW-14/F-04
   (mermaid securityLevel), QW-15/F-05 (DOMPurify cleanup).
3. Phase 03 - BOLA/IDOR matrix (local compose, two test accounts).
4. Phase 04 - input validation fuzz + rendering (local).
5. Phase 02 - auth/sessions (timing, fixation) (local).
6. Phase 08 - cargo audit / npm audit triage (local).
7. Phase 09 - logging/IR runbook + backup drill (local).
8. Phase 10 - remediation verification + regression tests in CI.

## Open findings summary
- Critical: F-01 (mitigated, verify)
- High: R-01, T-01, T-02, T-03, T-05, T-06 (server-verify)
- Medium: R-02, T-04, T-07, F-03, F-04
- Low/Info: F-02, R-03, R-07, F-05, F-06, R-04, R-05, R-06, R-08, R-09

## Environment notes
- Live host blocks IPs after rapid requests -> no automated scanning against live.
- ModSecurity (id:420043) on the shared LiteSpeed host bans after ~5 hits/3600s
  (logged 2026-11-09, IP <redacted-ip>). Live checks = single spaced `curl`
  only; high-volume testing uses local compose.yaml. See 01 §0.
- All active testing uses local compose.yaml.
- Disk ~2.1 GB free on /.

## Files
- Plan: specs/security/00..10, quick-wins, findings-register, orchestrator (this).
- Tasks: `specs/tasks/SEC-*.md` are forward-references only — stubs do not
  exist yet; create them when starting implementation. Shared tasks cover
  multiple findings (see findings-register.md): SEC-007 covers T-07 + F-04..F-06,
  SEC-009 covers R-01 + R-05, SEC-011 covers R-04 + F-03, SEC-014 covers F-01 + T-05.
- Code: backend-rs/, frontend/.

## Resume instructions
1. Read this file + findings-register.md.
2. Check `git status` (specs/security/ untracked).
3. Pick the next unchecked item.
4. Switch to Build mode before modifying files.