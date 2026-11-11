# 08 — Supply Chain & Dependencies

**Objective:** Eliminate known CVEs, verify lockfile integrity, and ensure build
reproducibility and action pinning.

**Scope:** `backend-rs/Cargo.toml` + `Cargo.lock`, `frontend/package.json` +
lockfile, `.github/workflows/backend-rs.yml`, Dockerfile, `deny.toml`.

---

## 1. Test plan

### 1.1 Rust
- `cargo audit` — triage all advisories. Every ignore in `deny.toml` must be
  bound to a task that removes it (per project convention).
- `cargo deny check advisories licenses bans`.
- Verify `Cargo.lock` committed; CI uses `--locked`.
- Check for unused direct deps carrying frameworks (already cleaned: axum-valid,
  utoipa-axum, zeroize removed).
- SBOM generation for release artifacts (cargo-sbom or syft).

### 1.2 Frontend (npm)
- `npm audit` (frontend) + `npm audit` (legacy backend).
- Triage high/critical.
- Verify lockfile committed and CI uses `npm ci`.
- Check for typosquatting / suspicious packages (mermaid, react-markdown,
  dompurify, radix-ui).

### 1.3 CI/CD
- All `uses:` pinned to commit SHAs (verified in workflow).
- `dtolnay/rust-toolchain` pinned to branch-tip SHA for 1.99.1.
- Secrets in GitHub Actions: none hardcoded; use encrypted secrets.
- Workflow triggers on workflow file changes (verified).
- Docker base images digest-pinned (verified).

### 1.4 Source secret scan
- `gitleaks` / `trufflehog` on full repo history.
- Verify `release/` (gitignored) not tracked; `.env` not tracked
  (verified: only `.env.example` tracked).

### 1.5 Dependency freshness policy
- Define cadence for `cargo update` / `npm update` with audit.
- Pin major versions; review changelogs for security-relevant changes.

## 2. Live findings so far
- Workflow already SHA-pinned + `--locked` (per T62). ✓
- Docker bases digest-pinned. ✓
- `.env` not tracked. ✓
- cargo audit/deny and npm audit not yet run in this audit — to be executed.

## 3. Remediation guidance
- Run audits, triage, create `SEC-*` tasks for each advisory.
- Add `cargo audit`/`npm audit` to CI if not present (cargo deny covers Rust).
- Add secret scanning to CI (gitleaks action).

## 4. Exit criteria
- Zero high/critical advisories open (or documented accepted risk with task).
- Secret scan clean.
- SBOM produced for release.

## 5. Task mapping
- `SEC-018` run cargo audit + npm audit, triage.
- `SEC-019` add gitleaks to CI.
- `SEC-020` SBOM for release artifacts.