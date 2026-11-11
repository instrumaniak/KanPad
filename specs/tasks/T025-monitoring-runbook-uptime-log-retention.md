# T025: Monitoring runbook — hosted uptime + log retention

> **STATUS: pending.** Wave 1. Closes **R8** (MEDIUM) per locked decision **D-4**
> (hosted uptime on `/ready` + JSON logs — no self-hosted Prometheus/Grafana).

## Goal

Downtime must not be user-discovered. Deliver an operations runbook + the external
prober setup:

1. **Uptime check:** hosted dead-man's-switch-style prober (healthchecks.io or
   equivalent free tier) hitting `GET /ready` every 1–5 min → alert on 2–3 consecutive
   failures + recovery ping. **Never probe `/metrics`** (unauthenticated,
   port-restricted) and don't confuse `/health` (DB-free liveness) with `/ready`
   (DB-aware readiness) — `/ready` is the correct probe.
2. **Log retention:** production already emits JSON to stdout (T60). Define **90 days**
   (spec gap SEC-021): logrotate example for Path A; host/cPanel rotation guidance for
   Path B.
3. **Alert threshold table** (document, not implement): `/ready` failing > 2 min; 5xx
   rate > 1% / 5 min (log-derived); disk > 85%; backup missed > 26 h (ties T011).

## Depends On

(None technically; T011's backup job is referenced by the thresholds — coordinate text
only.)

## Phase

1

## Critical

No — but it's the whole of R8 and cheap (D-4 chose the low-cost option).

## Spec References

- `specs/arch-hardening-specs/40-operations.md` §4 (monitoring design: probe choice,
  retention, thresholds, cost guardrail §8).
- `specs/arch-hardening-specs/00-overview.md` — finding R8; D-4; global DoD.
- `backend-rs/src/lib.rs:645-696` (`/health` vs `/ready` semantics), `backend-rs/.env.example:119`
  (metrics port warning), `specs/security/09-logging-monitoring-ir.md` (SEC-021/022/023
  gaps this closes), `docs/deployment.md` (post-deploy smoke list).

## Files to Create/Modify

| File | Change |
|---|---|
| `docs/operations-monitoring.md` | **NEW** — prober setup (both paths), alert threshold table, escalation notes, log retention policy + logrotate example, what NOT to probe |
| `docs/deployment.md` | One-line pointer to the monitoring runbook in the post-deploy smoke section |
| `specs/security/09-logging-monitoring-ir.md` | Annotate SEC-021 (retention defined) / related gaps as addressed — annotate, don't rewrite |
| `docs/lessons/ci-quality.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** app code, Prometheus/Grafana configs (D-4 excludes
them), real prober tokens (placeholders only).

## Implementation Steps

1. Verify `/ready` semantics from source (DB-aware, 500 ms budget) — cite lines in the
   runbook so the next agent doesn't re-derive them.
2. Write the runbook: prober setup steps (create check, URL, interval, alert contacts),
   recovery-ping config, the threshold table, log retention (90 d) + rotation examples
   for both paths, explicit "do not probe /metrics / do not use /health for DB alerts".
3. Coordinate with T011: add "backup missed > 26 h" to the table referencing its
   schedule; if T011 not landed, anchor with a TODO to its AC.
4. Optionally execute the prober setup against the live deployment (user's account) —
   evidence = the first successful ping; if credentials/account unavailable, deliver
   runbook + leave setup to user (record in §Notes).
5. Post-deploy smoke pointer in `docs/deployment.md`.
6. Spec annotation + lesson.

## Constraints

- D-4: no new self-hosted monitoring services (cost guardrail `40-operations.md` §8).
- Free tier only; no secrets committed (prober token = placeholder).
- Probes must not hit `/metrics` or trip the auth throttle (`/ready` is unthrottled by
  design — cite `lib.rs` route_layer note).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] `docs/operations-monitoring.md` lands with: `/ready` setup (both paths), interval
      + alert/recovery config, the four-row threshold table, 90-day retention +
      rotation examples, "don't probe /metrics" warning (closes **R8**).
- [ ] First successful external ping recorded (or explicit handoff note in §Notes).
- [ ] `docs/deployment.md` links the runbook; security spec gaps annotated.
- [ ] No tokens/secrets in the diff; lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
