//! T005 step 4: `serve` boot-time DB probe — fail fast on an unreachable
//! database, BEFORE the sweeper spawns, before bind, before
//! `--auto-migrate`.
//!
//! Three outcomes pinned here (the Step 0 BEFORE evidence generalized to
//! serve: an outage used to surface only as a causeless pool timeout on
//! the first request — or as `--auto-migrate` dying on `PoolTimedOut`
//! with no cause at all):
//!
//! * `DB_STARTUP_CHECK` default (require) + wrong password → exit **1**
//!   with the 1045 classification + the `DB_USERNAME` hint, ONE attempt,
//!   and stdout never carries the `backend-rs listening` line (the bind
//!   never happens);
//! * `DB_STARTUP_CHECK=warn` + wrong password → the process STAYS UP as a
//!   degraded server: `GET /health` → 200, `GET /ready` → 503 (the probe
//!   warning + the `continuing:` line on stderr) — the reason `warn`
//!   exists;
//! * `DB_STARTUP_CHECK=warn` + `--auto-migrate` + unreachable database →
//!   exit **1** with the explicit "does not exempt migrations" bail — a
//!   warn-mode server may serve degraded, but it never silently skips a
//!   migration the operator asked for.
//!
//! Black-box spawn plumbing comes from `super::migrate_cli`, imported
//! EXPLICITLY (the no-glob rule). This suite's own helpers are genuinely
//! suite-specific (a long-lived CHILD process + raw HTTP over a std
//! `TcpStream` — no shared helper spawns the binary as a server) and are
//! named outside the forbidden set: `ephemeral_port`, `spawn_serve`,
//! `http_get`. See `migrate_cli.rs`'s header for the sanctioned-exception
//! note covering this suite's reuse of its spawn plumbing.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::time::{Duration, Instant};

use super::migrate_cli::{TempCdir, cli_env, run_cli, shared_database_url};

/// A free port for the child's `PORT`: bind `:0`, read the OS-assigned
/// port, drop the listener. (The classic reserve-then-reuse race is
/// accepted — the OS hands out distinct ports far more often than a
/// parallel test could collide in the milliseconds before the child
/// binds; `bind_blackbox`'s in-process equivalent has the same property.)
fn ephemeral_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral bind");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

/// Spawn `backend-rs <args>` WITHOUT waiting (the serve child is
/// long-lived); the caller drives HTTP and then kills it. Same env rules
/// as [`super::migrate_cli::run_cli`]: `env_clear` + the explicit minimum
/// (+ `extra_env`), temp cwd so no developer `.env` is found.
fn spawn_serve(
    args: &[&str],
    database_url: &str,
    extra_env: &[(&str, &str)],
    cdir: &TempCdir,
) -> std::process::Child {
    let mut command = Command::new(super::migrate_cli::BIN);
    command
        .args(args)
        .current_dir(cdir.path())
        .env_clear()
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in cli_env(database_url) {
        command.env(key, value);
    }
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command.spawn().expect("spawn backend-rs serve")
}

/// One raw HTTP/1.1 GET over a std `TcpStream` — `(status, full body)`.
/// `None` when the socket refuses (the poll loop's "not up yet" signal).
/// `Connection: close` so the server closes after one response and the
/// read reaches EOF deterministically.
fn http_get(port: u16, path: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let status_line = raw.lines().next()?;
    let status: u16 = status_line.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, raw))
}

/// Poll `GET /health` until the degraded server answers (or the budget
/// expires) — the child's boot path is: settings load → probe (warn) →
/// sweeper → bind. A 1045 probe is ONE attempt (permanent),
/// so the budget is generous only against slow CI.
fn wait_for_health(port: u16, budget: Duration) -> u16 {
    let started = Instant::now();
    loop {
        if let Some((status, _)) = http_get(port, "/health") {
            return status;
        }
        if started.elapsed() > budget {
            panic!("the degraded serve child never answered /health within {budget:?}");
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// `require` (default) + wrong password → exit 1 with the classification,
/// the hint and ONE attempt, and the bind never happens (no `listening`
/// line in stdout; tracing logs to stdout, the bail goes to stderr).
#[test]
fn serve_require_bails_before_listening() {
    let Some((_, parts)) = shared_database_url() else {
        eprintln!("SKIP serve_require_bails_before_listening: DATABASE_URL unset or unparsable");
        return;
    };
    if !super::migrate_cli::reachable(&parts) {
        eprintln!("SKIP serve_require_bails_before_listening: the shared database is unreachable");
        return;
    }
    let wrong = format!(
        "mysql://{}:t005-wrong-password@{}:{}/{}?charset=utf8mb4&collation=utf8mb4_unicode_ci",
        parts.0, parts.2, parts.3, parts.4
    );
    let cdir = TempCdir::new("serve-require");
    let out = run_cli(&["serve"], &wrong, &[], &cdir);
    assert_eq!(out.status.code(), Some(1), "exit code stays 1");
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(err.contains("1045"), "classification in stderr: {err}");
    assert!(err.contains("access denied"), "class in stderr: {err}");
    assert!(err.contains("DB_USERNAME"), "the actionable hint: {err}");
    assert_eq!(
        super::migrate_cli::parse_attempts(&err),
        1,
        "a permanent class must fail on ONE attempt"
    );
    let out_text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        !out_text.contains("backend-rs listening"),
        "the bind must never happen: {out_text}"
    );
    // Neither the real nor the injected password may leak in either stream.
    assert!(!err.contains(&parts.1), "real password leaked: {err}");
    assert!(
        !err.contains("t005-wrong-password"),
        "wrong password echoed: {err}"
    );
    assert!(!out_text.contains(&parts.1));
    assert!(!out_text.contains("t005-wrong-password"));
}

/// `warn` + wrong password → the process STAYS UP degraded: /health 200,
/// /ready 503, and the `continuing:` line on stderr. This is the
/// outcome `DB_STARTUP_CHECK=warn` exists for — a DB-less server keeps
/// serving (and /ready says so) instead of a crash loop.
#[test]
fn serve_warn_mode_keeps_serving_degraded() {
    let Some((_, parts)) = shared_database_url() else {
        eprintln!("SKIP serve_warn_mode_keeps_serving_degraded: DATABASE_URL unset or unparsable");
        return;
    };
    if !super::migrate_cli::reachable(&parts) {
        eprintln!(
            "SKIP serve_warn_mode_keeps_serving_degraded: the shared database is unreachable"
        );
        return;
    }
    let wrong = format!(
        "mysql://{}:t005-wrong-password@{}:{}/{}?charset=utf8mb4&collation=utf8mb4_unicode_ci",
        parts.0, parts.2, parts.3, parts.4
    );
    let port = ephemeral_port();
    let cdir = TempCdir::new("serve-warn");
    let mut child = spawn_serve(
        &["serve"],
        &wrong,
        &[("DB_STARTUP_CHECK", "warn"), ("PORT", &port.to_string())],
        &cdir,
    );
    // Assert the degraded contract, THEN reap the child (kill + collect
    // both pipes — the pipes are small, the boot logs fit the 64 KiB
    // buffer, so no child-side write block can stall the poll).
    let health = wait_for_health(port, Duration::from_secs(20));
    assert_eq!(health, 200, "liveness must stay 200 while degraded");
    let (ready_status, ready_raw) =
        http_get(port, "/ready").expect("/ready must answer once /health does");
    assert_eq!(ready_status, 503, "readiness must say 503 while degraded");
    assert!(
        !ready_raw.contains("t005-wrong-password"),
        "the password must never reach the response: {ready_raw}"
    );

    let _ = child.kill();
    let output = child.wait_with_output().expect("reap the serve child");
    let err = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        err.contains("warning: cannot connect to the database"),
        "the probe warning on stderr: {err}"
    );
    assert!(
        err.contains("continuing: DB_STARTUP_CHECK=warn"),
        "the continuation notice on stderr: {err}"
    );
    assert!(
        !err.contains(&parts.1) && !err.contains("t005-wrong-password"),
        "no password on stderr: {err}"
    );
}

/// `warn` + `--auto-migrate` + unreachable database → exit 1 with the
/// explicit bail: warn buys a degraded server, never a silently-skipped
/// migration. (Refused port: no server involvement, so only a parsable
/// URL is required — the same independence
/// `refused_port_retries_within_the_budget` has.)
#[test]
fn serve_warn_auto_migrate_still_aborts_when_db_unreachable() {
    let Some((_, parts)) = shared_database_url() else {
        eprintln!("SKIP serve_warn_auto_migrate_still_aborts: DATABASE_URL unset or unparsable");
        return;
    };
    let refused = format!(
        "mysql://{}:{}@{}:1/{}?charset=utf8mb4&collation=utf8mb4_unicode_ci",
        parts.0, parts.1, parts.2, parts.4
    );
    let cdir = TempCdir::new("serve-warn-automigrate");
    let out = run_cli(
        &["serve", "--auto-migrate"],
        &refused,
        &[
            ("DB_STARTUP_CHECK", "warn"),
            ("DB_STARTUP_TIMEOUT_SECS", "2"),
        ],
        &cdir,
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "warn must NOT exempt --auto-migrate: {}/{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        err.contains("continuing: DB_STARTUP_CHECK=warn"),
        "the probe warned first: {err}"
    );
    assert!(
        err.contains("does not exempt migrations"),
        "the explicit warn+auto-migrate bail: {err}"
    );
    assert!(
        err.contains("connection refused"),
        "the cause stays visible: {err}"
    );
}
