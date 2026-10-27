//! T31 golden-fixture gate.
//!
//! Golden source: SPEC-DERIVED, not live Nest. Nest `:3000` was unreachable
//! when this gate was written (`curl` exit `000` on 2026-10-12), so per the
//! task brief the ~25 fixtures in `tests/golden/*.json` reconstruct Nest
//! responses from the spec instead (`20-api-contract.md`,
//! `21-api-standard-shapes.md`, `CONTRACT-CHANGES.md`). Each fixture documents its own `nest`
//! shape, the expected `rust` status/shape, and the intentional `deviation`
//! inline. If Nest is ever reachable again, re-capture `nest` bodies live and
//! re-verify; the Rust assertions below must still hold.
//!
//! What the gate pins (single `#[sqlx::test]` setup, sequential flow with one
//! owner; mirrors T21a/T21b style):
//! auth lifecycle (register 201 + `Location`, dup 409, weak-password 400 —
//! Rust-only per CONTRACT-CHANGES §6 — login 200, bad login 401, me 200,
//! logout 204, me-after-logout 401, re-login 200), project create 201,
//! board create 201 (+3 columns tx, `position ASC`), board detail 200
//! (nested, ordered), board list 200 (`Page`, items compared only),
//! card create/list/detail (list never selects TEXT), checklist + item
//! create, active-board delete 409, archive-flip + delete 204 (cascade
//! re-read), blank-name 400 (Rust-only), unknown-id 404, garbage-id 400,
//! `?page=0` 400 (Rust-only), and the deleted-route status table
//! (`/boards/archived` → 400, verb routes → 404).
//!
//! Comparison rules (per T31 AC): status + parsed-body SHAPE match,
//! order-insensitive where ordering is not contract (single-element arrays in
//! fixtures are per-item schemas, so list order never matters); `message`
//! strings, envelope wrappers, and pagination shape are excluded per contract
//! breakage — a Nest bare-array list vs Rust `Page<T>` compares on the `data`
//! items only. Ordering IS contract for nested board columns (`position ASC`)
//! and is asserted explicitly via the fixture `ordering` key.
//!
//! NO session wipe here: the `sessions` store table lives in THIS test's own
//! database (the store follows the pool's database — see
//! `AppState::with_pool`; see also `helpers.rs` module docs). This suite
//! mints its own user+jar and asserts nothing about absolute store counts.

use super::helpers::{app_on_pool, assert_problem, authed_request, cookie_jar, send};
use axum::http::{StatusCode, header};
use serde_json::Value;
use sqlx::MySqlPool;
use std::collections::HashMap;

fn lookup<'a>(body: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = body;
    for segment in path.split('.') {
        if let Ok(index) = segment.parse::<usize>() {
            current = current.as_array()?.get(index)?;
        } else {
            current = current.as_object()?.get(segment)?;
        }
    }
    Some(current)
}

/// Absent-path check; `*` fans out over array items
/// (`data.*.description` = no item carries `description`).
fn assert_absent(body: &Value, path: &str, what: &str) {
    fn walk(node: &Value, segments: &[&str]) -> bool {
        if segments.is_empty() {
            return true;
        }
        let (head, tail) = (segments[0], &segments[1..]);
        if head == "*" {
            let items = node.as_array().cloned().unwrap_or_default();
            return items.iter().any(|item| walk(item, tail));
        }
        match node.as_object().and_then(|map| map.get(head)) {
            None => false,
            Some(next) => {
                if tail.is_empty() {
                    return true;
                }
                walk(next, tail)
            }
        }
    }
    let segments: Vec<&str> = path.split('.').collect();
    assert!(
        !walk(body, &segments),
        "{what}: key path must be absent: {path} in {body}"
    );
}

/// Recursive shape assertion. Leaf tags: `@int` / `@string` / `@bool` /
/// `@any`. `{"@nullable": <shape>}` accepts null or the inner shape. Objects
/// require EXACT key sets (no extras — catches envelope/`message` regressions
/// beyond the `absent` list). Single-element arrays are per-item schemas, so
/// list order never matters (order-insensitive by construction); empty
/// literal arrays require an empty actual array.
fn assert_shape(actual: &Value, shape: &Value, at: &str, what: &str) {
    if let Some(tag) = shape.as_str() {
        let ok = match tag {
            "@int" => actual.is_i64() || actual.is_u64(),
            "@string" => actual.is_string(),
            "@bool" => actual.is_boolean(),
            "@any" => true,
            other => panic!("{what}: unknown shape tag {other} at {at}"),
        };
        assert!(ok, "{what}: expected {tag} at {at}, got {actual}");
        return;
    }
    if let Some(map) = shape.as_object() {
        if let Some(inner) = map.get("@nullable") {
            if actual.is_null() {
                return;
            }
            assert_shape(actual, inner, at, what);
            return;
        }
        let actual_map = actual.as_object().unwrap_or_else(|| {
            panic!("{what}: expected object at {at}, got {actual}");
        });
        let mut expected_keys: Vec<&String> = map.keys().collect();
        let mut actual_keys: Vec<&String> = actual_map.keys().collect();
        expected_keys.sort();
        actual_keys.sort();
        assert_eq!(
            actual_keys, expected_keys,
            "{what}: exact key set at {at} (extras or missing keys)"
        );
        for (key, sub) in map {
            assert_shape(&actual_map[key], sub, &format!("{at}.{key}"), what);
        }
        return;
    }
    if let Some(items) = shape.as_array() {
        let actual_items = actual.as_array().unwrap_or_else(|| {
            panic!("{what}: expected array at {at}, got {actual}");
        });
        if items.is_empty() {
            assert!(
                actual_items.is_empty(),
                "{what}: expected empty array at {at}, got {actual}"
            );
            return;
        }
        if items.len() == 1 {
            for (index, item) in actual_items.iter().enumerate() {
                assert_shape(item, &items[0], &format!("{at}[{index}]"), what);
            }
            return;
        }
        assert_eq!(
            actual_items.len(),
            items.len(),
            "{what}: multi-element literal array length at {at}"
        );
        for (index, (item, sub)) in actual_items.iter().zip(items.iter()).enumerate() {
            assert_shape(item, sub, &format!("{at}[{index}]"), what);
        }
        return;
    }
    assert_eq!(actual, shape, "{what}: literal mismatch at {at}");
}

/// `{"path": "data.columns", "key": "position", "dir": "asc"}` — ordering IS
/// contract here (board nested columns `position ASC`).
fn assert_ordering(body: &Value, spec: &Value, what: &str) {
    let path = spec["path"].as_str().expect("ordering.path");
    let key = spec["key"].as_str().expect("ordering.key");
    let dir = spec["dir"].as_str().expect("ordering.dir");
    let items = lookup(body, path)
        .unwrap_or_else(|| panic!("{what}: ordering path missing: {path} in {body}"))
        .as_array()
        .unwrap_or_else(|| panic!("{what}: ordering path not an array: {path}"))
        .clone();
    let mut values: Vec<i64> = items
        .iter()
        .map(|item| {
            item.get(key)
                .unwrap_or_else(|| panic!("{what}: ordering key missing: {key} in {item}"))
                .as_i64()
                .unwrap_or_else(|| panic!("{what}: ordering key not int: {key} in {item}"))
        })
        .collect();
    let mut sorted = values.clone();
    if dir == "asc" {
        sorted.sort();
    } else {
        sorted.sort_by(|a, b| b.cmp(a));
    }
    assert_eq!(values, sorted, "{what}: {path} not {dir} by {key}");
    values.dedup();
    assert_eq!(
        values.len(),
        items.len(),
        "{what}: duplicate {key} in {path} (positions must be dense)"
    );
}

/// Replace `{var}` path placeholders from the capture context.
fn resolve_path(template: &str, ctx: &HashMap<String, i64>, what: &str) -> String {
    let mut out = template.to_string();
    for (name, value) in ctx {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    assert!(
        !out.contains('{'),
        "{what}: unresolved placeholder in path: {out}"
    );
    out
}

/// Replace `{"$var": name}` body markers with captured integers (recursive).
fn resolve_body(template: &Value, ctx: &HashMap<String, i64>, what: &str) -> Value {
    match template {
        Value::Object(map) if map.len() == 1 && map.contains_key("$var") => {
            let name = map["$var"].as_str().expect("$var name");
            let value = ctx
                .get(name)
                .unwrap_or_else(|| panic!("{what}: unknown capture var {name}"));
            Value::from(*value)
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, val)| (key.clone(), resolve_body(val, ctx, what)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_body(item, ctx, what))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn golden_gate(pool: MySqlPool) {
    let app = || app_on_pool(&pool);

    let mut entries: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir("tests/golden")
        .expect("tests/golden exists")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .map(|path| {
            let raw = std::fs::read_to_string(&path).expect("golden reads");
            let parsed: Value = serde_json::from_str(&raw).expect("golden is JSON");
            let seq = parsed["seq"].as_u64().expect("golden has seq");
            (seq, path)
        })
        .collect();
    entries.sort();
    assert!(
        entries.len() >= 20,
        "gate needs ~20 goldens, found {}",
        entries.len()
    );

    let mut ctx: HashMap<String, i64> = HashMap::new();
    let mut jar = String::new();

    for (seq, path) in entries {
        let raw = std::fs::read_to_string(&path).expect("golden reads");
        let golden: Value = serde_json::from_str(&raw).expect("golden is JSON");
        let name = golden["name"]
            .as_str()
            .expect("golden has name")
            .to_string();
        let what = format!("#{seq} {name}");

        // Deleted-route status table: static router-level pins, one file.
        if let Some(cases) = golden.get("cases").and_then(Value::as_array) {
            for case in cases {
                let method = case["method"].as_str().expect("case method");
                let route = case["path"].as_str().expect("case path");
                let expected =
                    StatusCode::from_u16(case["rust_status"].as_u64().expect("case status") as u16)
                        .expect("valid status");
                let slug = case["problem"].as_str().expect("case problem");
                let body = if method == "POST" || method == "PATCH" {
                    Some(serde_json::json!({}).to_string())
                } else {
                    None
                };
                let (status, headers, bytes) =
                    send(app(), authed_request(method, route, Some(&jar), body)).await;
                let failure: Value = serde_json::from_slice(&bytes)
                    .unwrap_or_else(|_| panic!("{what}: {method} {route} not JSON"));
                assert_eq!(status, expected, "{what}: {method} {route}: {failure}");
                assert_problem(
                    status,
                    &headers,
                    &failure,
                    slug,
                    &format!("{what} {method} {route}"),
                );
            }
            continue;
        }

        let request = &golden["request"];
        let method = request["method"].as_str().expect("request method");
        let route = resolve_path(request["path"].as_str().expect("request path"), &ctx, &what);
        let authed = request["auth"].as_str().expect("request auth") == "owner";
        if authed {
            assert!(!jar.is_empty(), "{what}: owner jar must be minted by now");
        }
        let payload = request
            .get("body")
            .filter(|body| !body.is_null())
            .map(|body| resolve_body(body, &ctx, &what).to_string());
        let cookie = authed.then_some(jar.as_str());

        let (status, headers, bytes) =
            send(app(), authed_request(method, &route, cookie, payload)).await;

        let rust = &golden["rust"];
        let expected = StatusCode::from_u16(rust["status"].as_u64().expect("rust status") as u16)
            .expect("valid status");
        // 204 carries no body; every other response in the gate is JSON.
        let body: Value = if expected == StatusCode::NO_CONTENT {
            assert!(
                bytes.is_empty(),
                "{what}: 204 must carry no body, got {} bytes",
                bytes.len()
            );
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                panic!(
                    "{what}: response not JSON: {}",
                    String::from_utf8_lossy(&bytes)
                )
            })
        };
        assert_eq!(status, expected, "{what}: {method} {route}: {body}");

        if let Some(slug) = rust.get("problem").and_then(Value::as_str) {
            assert_problem(status, &headers, &body, slug, &what);
            if rust
                .get("require_errors")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                assert!(
                    body["errors"]
                        .as_array()
                        .is_some_and(|errors| !errors.is_empty()),
                    "{what}: must carry errors[]: {body}"
                );
            }
        }
        if let Some(shape) = rust.get("shape").filter(|shape| !shape.is_null()) {
            assert_shape(&body, shape, "$", &what);
        }
        for absent in rust
            .get("absent")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            assert_absent(&body, absent.as_str().expect("absent path"), &what);
        }
        if let Some(location) = rust.get("location_contains").and_then(Value::as_str) {
            let header_value = headers
                .get(header::LOCATION)
                .unwrap_or_else(|| panic!("{what}: 201 must set Location"))
                .to_str()
                .expect("Location is ASCII");
            assert!(
                header_value.contains(location),
                "{what}: Location {header_value} must contain {location}"
            );
        }
        if let Some(ordering) = rust.get("ordering").filter(|spec| !spec.is_null()) {
            assert_ordering(&body, ordering, &what);
        }

        for (var, pointer) in golden
            .get("capture")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
        {
            let value = lookup(&body, pointer.as_str().expect("capture pointer"))
                .unwrap_or_else(|| panic!("{what}: capture path missing: {pointer} in {body}"))
                .as_i64()
                .unwrap_or_else(|| panic!("{what}: capture not int: {pointer} in {body}"));
            ctx.insert(var, value);
        }
        if golden
            .get("capture_jar")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            jar = cookie_jar(&headers);
            assert!(!jar.is_empty(), "{what}: must set a session cookie");
        }

        // Seq 20 follow-up: the PATCH only archived. The gate additionally
        // pins the 204 delete inline (re-GET 404 + columns cascade gone via
        // SELECT) — the golden file documents the archive half.
        if name == "board-archive-then-delete-204" {
            let board_id = ctx.get("board_id").expect("board captured");
            let route = format!("/api/v1/boards/{board_id}");
            let (status, _, bytes) =
                send(app(), authed_request("DELETE", &route, Some(&jar), None)).await;
            assert!(bytes.is_empty(), "{what}: DELETE must be 204 with no body");
            assert_eq!(status, StatusCode::NO_CONTENT, "{what}: DELETE archived");
            let (status, headers, bytes) =
                send(app(), authed_request("GET", &route, Some(&jar), None)).await;
            let failure: Value = serde_json::from_slice(&bytes).expect("404 is JSON");
            assert_eq!(status, StatusCode::NOT_FOUND, "{what}: re-GET: {failure}");
            assert_problem(status, &headers, &failure, "not-found", &what);
            let columns: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM `columns` WHERE `board_id` = ?")
                    .bind(board_id)
                    .fetch_one(&pool)
                    .await
                    .expect("cascade re-read");
            assert_eq!(columns, 0, "{what}: columns cascade gone");
        }
    }

    // NO TEARDOWN WIPE/DROP: the shared store must survive until every suite
    // in this process is done.
}
