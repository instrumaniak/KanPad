# 03 — Authorization & Ownership (BOLA/IDOR)

**Objective:** Ensure every resource endpoint enforces per-user ownership (404 for
foreign IDs, no oracle) and role checks (403 only for role denial), with no mass
assignment.

**Scope:** all feature stores/routes: projects, boards, columns, cards, labels,
checklists, items, notes, tags. Ownership helpers in `common/ownership.rs`.

**Threat actors:** authenticated user A attempting to access/modify user B's data;
low-privilege user attempting admin actions.

---

## 1. Test plan

### 1.1 Horizontal access (user A → user B's resources)
For each resource type, with two accounts, verify:
- `GET /api/v1/<res>/{id}` (B's id) → 404.
- `PATCH /api/v1/<res>/{id}` (B's id) → 404, no mutation.
- `DELETE /api/v1/<res>/{id}` (B's id) → 404 (or 204 only if owned).
- Nested: `GET /api/v1/columns/{colId}/cards` where col belongs to B → 404.
- `GET /api/v1/cards/{cardId}/checklists` where card belongs to B → 404.
- `PUT /api/v1/cards/{id}/labels/{label_id}` where label belongs to B → 404.
- `POST /api/v1/cards/{id}/labels` attaching B's label to A's card → 404.
- `POST /api/v1/columns/{id}/move-cards` across boards → ownership on both.
- `POST /api/v1/boards/{boardId}/columns` creating column in B's board → 404.

Resources: projects, boards, columns, cards, labels, checklists, checklist-items,
notes, tags.

### 1.2 Vertical access (role)
- Any admin-only route (if present) → 403 for non-admin, 404 for foreign.
- `GET /api/v1/auth/me` for a demoted user → 403; deleted user → 401.

### 1.3 Mass assignment / parameter tampering
- Register/update payloads with extra fields: `role`, `user_id`, `id`,
  `is_archived`, `position` (where not allowed) → ignored (serde
  ignore-unknown-fields). Verify no field leaks into persisted row.
- `POST /api/v1/boards` with `project_id` pointing at B's project → 404/403
  (project ownership checked), not silently attached.
- `PATCH /api/v1/cards/{id}` changing `board_id`/`column_id` to B's → rejected
  or ownership-checked.

### 1.4 List endpoints (scoping)
- `GET /api/v1/boards`, `/projects`, `/labels`, `/notes`, `/tags` return only the
  caller's rows (no cross-user leakage in totals or items).

### 1.5 Code review (white-box)
- grep every store.rs for ownership predicates (`WHERE ...user_id = ?` /
  `board.user_id` / `project.user_id`).
- Verify `common/ownership.rs` is the single owner of scoping helpers (no
  duplicated, possibly-wrong predicates).
- Verify foreign IDs map to 404, not 403 (no oracle).

## 2. Live findings so far
- None yet (requires two accounts + code review). Endpoints inherit session auth
  (OpenAPI shows no public data routes except auth register/login, health, ready).

## 3. Remediation guidance
- If any endpoint lacks an ownership predicate: add `...WHERE board.user_id = ?`
  (or join) and return `AppError::NotFound` on miss.
- If mass assignment leaks: ensure DTOs only deserialize allowed fields; never
  bind request body directly to an entity with relation fields.
- Add a cross-user 404 test per resource to CI as a regression guard.

## 4. Exit criteria
- All 1.1–1.4 pass for every resource type.
- Code review confirms ownership predicates present in every store fn that reads/
  mutates user-scoped data.
- Regression tests added to CI.

## 5. Task mapping
- `SEC-004` cross-user access audit + fixes.
- `SEC-005` mass-assignment hardening + DTO review.