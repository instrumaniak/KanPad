//! T005 step 2: `migrate` CLI connectivity outcomes — the suite that keeps
//! `migrate info` from ever printing "fresh database" for an outage.
//!
//! **Step 0 evidence this suite pins down** (the BEFORE, captured on the
//! unpatched tree): a wrong password or a refused port made `migrate info`
//! print `applied: 0 pending: 2` / "fresh database" and exit **0**. The
//! AFTER asserts the four outcomes that matter:
//!
//! * wrong password → exit 1, `1045` / "access denied" + the `DB_USERNAME`
//!   hint, ONE attempt (permanent classes never burn the retry budget), and
//!   stdout NEVER contains `pending:` (the old bogus output);
//! * refused port → exit 1, "connection refused" + the `DB_PORT` hint, and
//!   MORE than one attempt within `DB_STARTUP_TIMEOUT_SECS` (the transient
//!   retry actually fires);
//! * unknown database → exit 1, `1049` + the `CREATE DATABASE` hint;
//! * healthy database → `migrate run` then `migrate info` end with the
//!   load-bearing last line `applied: <all> pending: 0`, and a suite-owned
//!   fresh database reports "journal: no _sqlx_migrations table yet (fresh
//!   database)" + `applied: 0 pending: <all>` with exit 0.
//!
//! Suite-owned databases only: the shared `DATABASE_URL` database (the 3337
//! tmpfs container) restarts to "fresh database" at any moment, so asserting
//! `pending: 0` against it is order-dependent flake. The suite derives
//! `kanpad_cli_<pid>` / `kanpad_cli_<pid>_fresh` from the env-provided URL
//! (path-segment swap; valid identifiers per `db::is_valid_db_name`).
//!
//! Skip+pass when `DATABASE_URL` is absent, unparsable, or unreachable —
//! mirroring `health.rs::ready_200_when_db_up` (CI always provides a live
//! one; a local run without MySQL still exercises the deterministic core).
//!
//! Harness rule: request plumbing comes from `super::helpers`, imported
//! EXPLICITLY (a glob import would let a leftover local definition shadow
//! the shared one silently — the E0255 compile error is the guard). This
//! suite drives the REAL BINARY, which no shared helper does
//! (`create_admin.rs` drives `run_create_admin` in-process), so its spawn
//! plumbing is genuinely suite-specific and named outside the forbidden set:
//! `bin`, `run_cli`, `cli_env`, `TempCdir`, `shared_database_url`,
//! `database_parts`, `suite_url`, `reachable`, `create_database`,
//! `drop_database`, `parse_attempts`, `migration_count`.
//!
//! SANCTIONED CONSUMER: `serve_boot.rs` (T005 step 4) drives the REAL
//! BINARY as a long-lived SERVER (`serve` + degraded-mode probes), which is
//! the same no-shared-helper situation — it imports this suite's spawn
//! plumbing EXPLICITLY (`TempCdir`, `cli_env`, `run_cli`,
//! `shared_database_url`, `reachable`, `parse_attempts`, `BIN`) instead of
//! re-declaring any of it. If a serve-side need outgrows that list, add a
//! NEW name here or in serve_boot — never a copy.

use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use sqlx::MySqlPool;

/// The compile-time binary path (`CARGO_BIN_EXE_<name>`; bin target
/// `backend-rs`). Integration tests only: cargo sets it for `tests/`.
pub(crate) const BIN: &str = env!("CARGO_BIN_EXE_backend-rs");

/// The session secret every spawned CLI needs (`SESSION_SECRET` ≥ 32 bytes)
/// — explicitly injected so the test never depends on a developer `.env`.
const SESSION_SECRET: &str = "forty-two-bytes-of-test-session-secret!!";

/// Parsed `DATABASE_URL` parts. The MySQL password getter is `pub(crate)`
/// on `MySqlConnectOptions` (sqlx 0.9), so the suite parses the URL
/// manually — with the SAME segmentation rules sqlx uses: the LAST `@`
/// splits userinfo from host (a userinfo password is percent-encoded and
/// cannot contain a raw `@`), the first `:` splits user from password, the
/// FIRST `/` separates host:port from the database, the LAST `:` splits
/// host from port, and everything after `?` is options.
type Parts = (String, String, String, u16, String);

/// `(mysql://user:pass@host:port/db?…)` → `(user, pass, host, port, db)`.
pub(crate) fn database_parts(url: &str) -> Option<Parts> {
    let rest = url.strip_prefix("mysql://")?;
    let (auth, tail) = rest.rsplit_once('@')?;
    let (user, pass) = auth.split_once(':')?;
    let (hostport, db_and_options) = tail.split_once('/')?;
    let (host, port) = hostport.rsplit_once(':')?;
    let (db, _) = db_and_options
        .split_once('?')
        .unwrap_or((db_and_options, ""));
    Some((
        user.to_string(),
        pass.to_string(),
        host.to_string(),
        port.parse().ok()?,
        db.to_string(),
    ))
}

/// The env-provided shared `DATABASE_URL` + its parsed parts — `None` when
/// absent or unparsable (the suite skips and passes).
pub(crate) fn shared_database_url() -> Option<(String, Parts)> {
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|url| !url.is_empty())?;
    let parts = database_parts(&url)?;
    Some((url, parts))
}

/// A derived suite URL: same credentials/host, swapped database name, and
/// the REQUIRED charset + collation pins (`Settings::validate` rejects
/// their absence). The password round-trips raw — the same characters the
/// shared URL carried, so the rebuild cannot diverge from the input.
pub(crate) fn suite_url(parts: &Parts, database: &str) -> String {
    format!(
        "mysql://{}:{}@{}:{}/{database}?charset=utf8mb4&collation=utf8mb4_unicode_ci",
        parts.0, parts.1, parts.2, parts.3
    )
}

/// Can the test process reach host:port at all? A 2 s TCP connect — the
/// skip gate for tests whose assertions need the SERVER to answer (1045 and
/// 1049 come from the server, not the transport).
pub(crate) fn reachable(parts: &Parts) -> bool {
    let Ok(mut addrs) = (parts.2.as_str(), parts.3).to_socket_addrs() else {
        return false;
    };
    let Some(address) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&address, Duration::from_secs(2)).is_ok()
}

/// Env for the spawned CLI: `env_clear` first (a developer shell may carry
/// stale `DB_*`/`RUST_LOG`), then the explicit minimum. PATH is restored so
/// the child resolves shared libraries the way a normal spawn would.
pub(crate) fn cli_env(database_url: &str) -> Vec<(String, String)> {
    vec![
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_default(),
        ),
        ("APP_ENV".to_string(), "test".to_string()),
        ("SESSION_SECRET".to_string(), SESSION_SECRET.to_string()),
        ("DATABASE_URL".to_string(), database_url.to_string()),
    ]
}

/// Temp `current_dir` for the child, removed on drop. Keeps
/// `dotenvy::dotenv()` from finding any `.env` (it searches CWD and
/// ancestors — a temp dir under /tmp has none), on top of `env_clear`.
pub(crate) struct TempCdir(PathBuf);

impl TempCdir {
    pub(crate) fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("kanpad-migrate-cli-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp cwd created");
        Self(dir)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempCdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Spawn `backend-rs <args>` with the explicit env (+ extra overrides),
/// blocking until exit (the CLI is short-lived: worst case the retry
/// budget). Panics propagate as test failures.
pub(crate) fn run_cli(
    args: &[&str],
    database_url: &str,
    extra_env: &[(&str, &str)],
    cdir: &TempCdir,
) -> Output {
    let mut command = Command::new(BIN);
    command.args(args).current_dir(cdir.path()).env_clear();
    for (key, value) in cli_env(database_url) {
        command.env(key, value);
    }
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command.output().expect("spawn backend-rs")
}

pub(crate) fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub(crate) fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// `(attempts: N` out of the probe-failure bail line — the AC evidence that
/// the retry policy ran (wrong password → 1, refused port → > 1).
pub(crate) fn parse_attempts(stderr: &str) -> u32 {
    let marker = "(attempts: ";
    let start = stderr.find(marker).expect("bail line carries (attempts: N") + marker.len();
    let rest = &stderr[start..];
    let end = rest.find(',').expect("comma after the attempt count");
    rest[..end]
        .trim()
        .parse()
        .expect("attempt count is a number")
}

/// Number of migration files — the suite asserts `applied: <N> pending: 0`
/// against THIS count instead of a hardcoded `2`, so adding a migration
/// later updates the expectation automatically (the invariant is "all
/// applied, none pending", not the literal 2).
pub(crate) fn migration_count() -> usize {
    std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
        .expect("migrations dir readable")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".sql"))
        .count()
}

/// Pool to the SHARED database (the one the env URL names) — used only for
/// suite-DB setup/teardown (`CREATE DATABASE` cannot take a bound
/// parameter, so the suite-generated names are interpolated inside
/// backticks; they are validated identifiers by construction).
async fn admin_pool(parts: &Parts) -> Result<MySqlPool, sqlx::Error> {
    MySqlPool::connect(&suite_url(parts, &parts.4)).await
}

async fn create_database(pool: &MySqlPool, name: &str) {
    // DDL identifiers cannot be bound parameters; sqlx 0.9 requires
    // `&'static str` for plain `query()`, so interpolated SQL goes through
    // the same `AssertSqlSafe` pattern `cards_move.rs` uses (T61). The
    // names are suite-generated (`kanpad_cli_<pid>…`) and validated
    // identifier shapes — never user input.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )))
    .execute(pool)
    .await
    .unwrap_or_else(|err| panic!("CREATE DATABASE {name}: {err}"));
}

async fn drop_database(pool: &MySqlPool, name: &str) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS `{name}`"
    )))
    .execute(pool)
    .await
    .unwrap_or_else(|err| panic!("DROP DATABASE {name}: {err}"));
}

/// The last stdout line — `migrate info`'s status line is load-bearing
/// (`docs/data-migration-live.md` gates grep it), so it must be EXACTLY
/// last.
fn last_line(text: &str) -> &str {
    text.lines().next_back().unwrap_or("").trim_end()
}

/// Wrong password → exit 1 with the 1045 classification + `DB_USERNAME`
/// hint, ONE attempt (permanent classes must not burn the retry budget),
/// and stdout never carries the old bogus `pending:` output.
#[test]
fn wrong_password_fails_with_1045_and_never_prints_pending() {
    let Some((_, parts)) = shared_database_url() else {
        eprintln!("SKIP wrong_password_fails_with_1045: DATABASE_URL unset or unparsable");
        return;
    };
    if !reachable(&parts) {
        eprintln!("SKIP wrong_password_fails_with_1045: the shared database is unreachable");
        return;
    }
    let wrong = format!(
        "mysql://{}:t005-wrong-password@{}:{}/{}?charset=utf8mb4&collation=utf8mb4_unicode_ci",
        parts.0, parts.2, parts.3, parts.4
    );
    let cdir = TempCdir::new("badpw");
    let out = run_cli(&["migrate", "info"], &wrong, &[], &cdir);
    assert_eq!(out.status.code(), Some(1), "exit code stays 1");
    let err = stderr(&out);
    assert!(err.contains("1045"), "1045 in stderr: {err}");
    assert!(err.contains("access denied"), "class in stderr: {err}");
    assert!(
        err.contains("DB_USERNAME"),
        "the actionable hint in stderr: {err}"
    );
    assert_eq!(
        parse_attempts(&err),
        1,
        "a permanent class must fail on ONE attempt"
    );
    let out_text = stdout(&out);
    assert!(
        !out_text.contains("pending:"),
        "the OLD bogus output must never appear: {out_text}"
    );
    // Neither the REAL password nor the injected wrong one may leak.
    assert!(!err.contains(&parts.1), "real password leaked: {err}");
    assert!(
        !err.contains("t005-wrong-password"),
        "wrong password echoed: {err}"
    );
    assert!(!out_text.contains("t005-wrong-password"));
}

/// Refused port (host valid, port 1) → exit 1, "connection refused" + the
/// `DB_PORT` hint, and MORE than one attempt within the budget: the
/// transient retry actually fires (the Step 0 BEFORE exited 0 after one
/// silent pool retry and printed "fresh database").
#[test]
fn refused_port_retries_within_the_budget_and_names_the_db_port_hint() {
    let Some((_, parts)) = shared_database_url() else {
        eprintln!("SKIP refused_port_retries_within_the_budget: DATABASE_URL unset or unparsable");
        return;
    };
    let refused = format!(
        "mysql://{}:{}@{}:1/{}?charset=utf8mb4&collation=utf8mb4_unicode_ci",
        parts.0, parts.1, parts.2, parts.4
    );
    let cdir = TempCdir::new("refused");
    let out = run_cli(
        &["migrate", "info"],
        &refused,
        &[("DB_STARTUP_TIMEOUT_SECS", "2")],
        &cdir,
    );
    assert_eq!(out.status.code(), Some(1), "exit code stays 1");
    let err = stderr(&out);
    assert!(
        err.contains("connection refused"),
        "transport code in stderr: {err}"
    );
    assert!(
        err.contains("DB_PORT"),
        "the actionable hint in stderr: {err}"
    );
    assert!(
        parse_attempts(&err) > 1,
        "a refused port must RETRY within the budget: {err}"
    );
    let out_text = stdout(&out);
    assert!(
        !out_text.contains("pending:"),
        "the OLD bogus output must never appear: {out_text}"
    );
}

/// Unknown database (never created) → exit 1 with 1049 + the
/// `CREATE DATABASE` hint — one attempt (1049 is permanent).
#[test]
fn unknown_database_fails_with_1049_and_the_create_database_hint() {
    let Some((_, parts)) = shared_database_url() else {
        eprintln!("SKIP unknown_database_fails_with_1049: DATABASE_URL unset or unparsable");
        return;
    };
    if !reachable(&parts) {
        eprintln!("SKIP unknown_database_fails_with_1049: the shared database is unreachable");
        return;
    }
    let missing = format!("kanpad_cli_missing_{}", std::process::id());
    let missing_url = suite_url(&parts, &missing);
    let cdir = TempCdir::new("unknown-db");
    let out = run_cli(&["migrate", "info"], &missing_url, &[], &cdir);
    assert_eq!(out.status.code(), Some(1), "exit code stays 1");
    let err = stderr(&out);
    assert!(err.contains("1049"), "1049 in stderr: {err}");
    assert!(
        err.contains("CREATE DATABASE"),
        "the actionable hint in stderr: {err}"
    );
    assert_eq!(parse_attempts(&err), 1, "1049 is permanent — one attempt");
    let out_text = stdout(&out);
    assert!(
        !out_text.contains("pending:"),
        "the OLD bogus output must never appear: {out_text}"
    );
}

/// The healthy path end-to-end on SUITE-OWNED databases:
///
/// * `migrate run` creates the journal → `applied: <all>`;
/// * `migrate info` prints the target line + the exact last line
///   `applied: <all> pending: 0`;
/// * a FRESH suite-owned database reports "journal: no _sqlx_migrations
///   table yet (fresh database)" + `applied: 0 pending: <all>` with exit 0 —
///   the ONLY path that may print the fresh-database message.
#[tokio::test]
async fn migrate_run_then_info_reports_exact_counts_on_suite_owned_databases() {
    let Some((_, parts)) = shared_database_url() else {
        eprintln!(
            "SKIP migrate_run_then_info_reports_exact_counts: DATABASE_URL unset or unparsable"
        );
        return;
    };
    let pool = match admin_pool(&parts).await {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!(
                "SKIP migrate_run_then_info_reports_exact_counts: shared database unreachable ({err})"
            );
            return;
        }
    };
    let total = migration_count();
    assert!(total > 0, "the crate ships migrations");

    let main_db = format!("kanpad_cli_{}", std::process::id());
    let fresh_db = format!("{main_db}_fresh");
    create_database(&pool, &main_db).await;
    create_database(&pool, &fresh_db).await;
    let main_url = suite_url(&parts, &main_db);
    let fresh_url = suite_url(&parts, &fresh_db);
    let cdir = TempCdir::new("healthy");

    // 1. `migrate run` applies everything on the (empty) suite database.
    let out = run_cli(&["migrate", "run"], &main_url, &[], &cdir);
    assert!(
        out.status.success(),
        "migrate run on a fresh suite DB: {} / {}",
        stdout(&out),
        stderr(&out)
    );
    assert_eq!(
        last_line(&stdout(&out)),
        format!("migrations applied: {total}"),
        "run's last line counts every migration"
    );

    // 2. `migrate info` on the migrated DB: target line first, exact last
    //    line `applied: N pending: 0`.
    let out = run_cli(&["migrate", "info"], &main_url, &[], &cdir);
    assert!(
        out.status.success(),
        "migrate info on a migrated DB: {} / {}",
        stdout(&out),
        stderr(&out)
    );
    let out_text = stdout(&out);
    let expected_target = format!(
        "database: {main_db} at {}:{} (user {:?}, source DATABASE_URL)",
        parts.2, parts.3, parts.0
    );
    assert!(
        out_text.contains(&expected_target),
        "the target line names the inspected database: {out_text}"
    );
    assert_eq!(
        last_line(&out_text),
        format!("applied: {total} pending: 0"),
        "the load-bearing last line"
    );

    // 3. `migrate info` on a FRESH database: the fresh-database message is
    //    legitimate ONLY here (the DB answers; only the journal is absent).
    let out = run_cli(&["migrate", "info"], &fresh_url, &[], &cdir);
    assert!(
        out.status.success(),
        "migrate info on a fresh DB must exit 0: {} / {}",
        stdout(&out),
        stderr(&out)
    );
    let out_text = stdout(&out);
    assert!(
        out_text.contains("journal: no _sqlx_migrations table yet (fresh database)"),
        "fresh-database message: {out_text}"
    );
    assert_eq!(
        last_line(&out_text),
        format!("applied: 0 pending: {total}"),
        "every migration is pending on a fresh database"
    );

    // Teardown (assert failures above leak the two uniquely-named DBs into
    // the tmpfs container — harmless, wiped on restart).
    drop_database(&pool, &main_db).await;
    drop_database(&pool, &fresh_db).await;
}

/// T005 step 5: an unparseable `DATABASE_URL` is a CONFIG error, not an
/// outage — `Settings::load` rejects it (`validate`), and `create-admin`
/// exits with `AdminExit::Db` (3) after printing the message that names
/// `DATABASE_URL`. BEFORE this change a malformed URL silently degraded the
/// pool to an unroutable `127.0.0.1:1`; now boot refuses it outright.
/// (`serve` and `migrate *` take the same rejection — exit 1 — this test
/// pins the create-admin variant because its exit code is distinct.)
#[test]
fn unparseable_database_url_fails_create_admin_with_exit_3() {
    let cdir = TempCdir::new("unparseable");
    let out = run_cli(
        &[
            "create-admin",
            "--email",
            "t005@example.test",
            "--password",
            "irrelevant-the-config-fails-first",
        ],
        "://not a url",
        &[],
        &cdir,
    );
    assert_eq!(out.status.code(), Some(3), "AdminExit::Db is exit 3");
    let err = stderr(&out);
    assert!(
        err.contains("DATABASE_URL"),
        "the message must name the variable: {err}"
    );
    assert!(
        err.contains("percent-encoded"),
        "the actionable hint must reach the operator: {err}"
    );
    let out_text = stdout(&out);
    assert!(
        !out_text.contains("://not a url") && !err.contains("://not a url"),
        "the raw URL must never be echoed: {err} / {out_text}"
    );
}
