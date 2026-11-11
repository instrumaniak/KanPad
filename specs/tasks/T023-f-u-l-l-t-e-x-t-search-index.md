# T023: FULLTEXT search index (conditional)

> **STATUS: pending — CONDITIONAL.** Wave 1. Closes the search half of **P5** (MEDIUM).
> Execute only when data volume warrants (T026 informs; user confirms). If deferred,
> record the decision in §Notes — never silently skip.

## Goal

Notes/projects search is `LIKE '%q%'` (escaped, bound — safe) leading-wildcard, so
`IDX_notes_title` can never serve it → table/index scan + row filter on every keystroke
at 100+ users × real data volume.

Switch to InnoDB **FULLTEXT** (`MATCH … AGAINST (? IN BOOLEAN MODE)`) on
`notes.title` (+ `notes.content`? — product open call) and `projects.name`, keeping
`LIKE` as fallback for terms FTS can't tokenize (short/stopword-only).

## Depends On

(None technically; practically gated on T026 data-volume evidence + user go-ahead.)

## Phase

1

## Critical

No — conditional; LIKE is correct, just O(scan).

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Search FTS (design: migration,
  boolean-mode operator neutralization, min-token-size caveat, ranking-vs-recency,
  MySQL/MariaDB parity, edge cases).
- `specs/arch-hardening-specs/00-overview.md` — finding P5; global DoD.
- `backend-rs/src/features/notes/store.rs:344-357` (LIKE + join shape),
  `backend-rs/src/features/projects/store.rs:42-66`, `migrations/0001_baseline.sql:183`
  (`IDX_notes_title`), existing emoji gate test pattern (T02b) for unicode parity.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/000N_*.sql` | **NEW** — `FULLTEXT INDEX` on `notes.title` (+content?), `projects.name` (online DDL, both engines) |
| `backend-rs/src/features/{notes,projects}/store.rs` | FTS query path + LIKE fallback; same escaping discipline |
| `backend-rs/.sqlx/` | Regen (pinned CLI, MySQL, db `kanpad`) |
| `backend-rs/tests/api/` | FTS-vs-LIKE parity on seeded corpus; operator-injection inputs; emoji/CJK roundtrip; both engines |
| `specs/api-rewrite-rs-specs/30-db-schema.md` | Index-list amendment (T52 if contract-visible; schema doc at minimum) |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** server config changes (`innodb_ft_min_token_size` —
not portable; document instead), relevance-ranking UX changes (default = keep recency
order), other search surfaces.

## Implementation Steps

TDD.

1. **Decision gate:** confirm with user + T026 evidence (or explicit "data is small,
   defer" note). If deferred → this task closes as "deferred, recorded".
2. **Red parity test:** seed a corpus (incl. partial matches, unicode, operator chars)
   → assert FTS results ⊇/≡ LIKE results modulo ranking.
3. Migration: FULLTEXT on both columns; apply on MySQL 8.4 **and** MariaDB 10.6 (CI
   matrix); verify online-DDL safety notes (InnoDB FTS index build locks — document).
4. Query path: boolean mode with **operator neutralization** (strip/escape `+ - * ( )`
   " etc. from user input); fallback to LIKE when token unusable; `ORDER BY updated_at`
   (recency — current behavior) unless ranking decided otherwise in §Notes.
5. Injection + unicode tests (reuse emoji gate pattern); existing notes/projects search
   tests green or updated with rationale.
6. Gates both engines; `sqlx prepare --check`; spec amendment; lesson.

## Constraints

- Both engines green (global DoD) — FTS behavior differs (stopwords, min token size);
  parity tests are the proof.
- User input never reaches MATCH un-neutralized (test operator strings).
- No server-config assumptions; document `innodb_ft_min_token_size=3` behavior.
- Migration live-safe (no table copy surprises — verify EXPLAIN/ALGORITHM on both).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Decision recorded: execute now or defer (with reason) — no silent skip.
- [ ] If executed: FULLTEXT migration green on MySQL + MariaDB; FTS-vs-LIKE parity test
      green; operator-injection input safe; emoji/CJK roundtrip green (closes search
      half of **P5**).
- [ ] LIKE fallback works for untokenizable terms; recency ordering preserved (or
      ranking decision documented).
- [ ] `sqlx prepare --check` 0; spec index amendment; lesson appended.

## Notes

(filled in during/after implementation)

**Open calls:** (1) include `notes.content` in the index? *Default: title only for
v1 (content search = product decision).* (2) ranking vs recency. *Default: recency.*

**Deferral record:** (if T026/user defers) date + reason + trigger for revisit
(e.g. "revisit when notes > 10k rows or p95 search > 200 ms").

## Lessons

(pending)
