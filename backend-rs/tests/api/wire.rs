//! Shared black-box HTTP/1.1 wire helpers (T50a + T50c own the two suites
//! that use this).
//!
//! # Ownership (NORMATIVE — import, never redefine)
//! The two black-box suites drive the real app over a raw TCP socket, so they
//! cannot use the in-process harness in [`super::helpers`]. Everything that
//! is transport-independent lives in ONE place here:
//!
//! [`WireResponse`] · [`parse_response`] · [`dechunk`] · [`header_first`] ·
//! [`cookie_jar`] · [`assert_problem`] · [`bind_blackbox`].
//!
//! (`WireResponse::json` is the wire-flavored body parser; the in-process
//! counterpart is `super::helpers::json_of`.)
//!
//! Import EXPLICITLY (`use super::wire::{WireResponse, parse_response};`),
//! never `use super::wire::*` — a glob import lets a leftover local
//! definition silently shadow the shared one (see the same rule in
//! `helpers.rs`).
//!
//! The problem+json assertion itself is NOT duplicated here: it delegates to
//! `super::helpers::assert_problem_fields`, the same core the in-process
//! suites use, so the two transports can never drift on what a problem body
//! must contain. The cookie-jar extraction is the same story —
//! `wire::cookie_jar` delegates to `super::helpers::cookie_jar_from`.
//!
//! What stays per-suite (deliberately): the socket client itself. T50a speaks
//! async `tokio::net::TcpStream` and keeps a name/value cookie jar; T50c
//! speaks a blocking `std::net::TcpStream` and derives the jar from the
//! response headers on demand. Only the byte-level protocol code is shared.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

use serde_json::Value;

use super::helpers::assert_problem_fields;

/// A parsed HTTP/1.1 response.
#[derive(Debug, Clone)]
pub(crate) struct WireResponse {
    /// Status code.
    pub status: u16,
    /// Header names are lowercased; each may repeat (`Set-Cookie`).
    pub headers: HashMap<String, Vec<String>>,
    /// Decoded body bytes (chunked encoding already undone).
    pub body: Vec<u8>,
}

impl WireResponse {
    /// First value of `name` (header names are matched lowercased).
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Parse the body as JSON, panicking with `what` on a non-JSON body.
    ///
    /// The wire-flavored counterpart of `super::helpers::json_of` — same
    /// contract, different transport (bytes off a socket, not an axum body).
    pub(crate) fn json(&self, what: &str) -> Value {
        assert!(
            !self.body.is_empty(),
            "{what}: expected a JSON body, got empty"
        );
        serde_json::from_slice(&self.body).unwrap_or_else(|err| {
            panic!(
                "{what}: response is not JSON (status {}): {err}: {}",
                self.status,
                String::from_utf8_lossy(&self.body)
            )
        })
    }
}

/// Parse a raw HTTP/1.1 response.
///
/// `method`/`path` only label panic messages. A `transfer-encoding: chunked`
/// body is de-chunked; anything else is taken verbatim.
pub(crate) fn parse_response(raw: &[u8], method: &str, path: &str) -> WireResponse {
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("{method} {path}: malformed HTTP response"));
    let head = std::str::from_utf8(&raw[..split])
        .unwrap_or_else(|_| panic!("{method} {path}: response head is not UTF-8"));
    let mut lines = head.lines();
    let status_line = lines.next().expect("status line present");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("status code present")
        .parse()
        .expect("status code numeric");
    let mut headers: HashMap<String, Vec<String>> = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers
                .entry(name.trim().to_ascii_lowercase())
                .or_default()
                .push(value.trim().to_string());
        }
    }
    let body_raw = &raw[split + 4..];
    let chunked = headers.get("transfer-encoding").is_some_and(|values| {
        values
            .iter()
            .any(|value| value.to_ascii_lowercase().contains("chunked"))
    });
    let body = if chunked {
        dechunk(body_raw)
    } else {
        body_raw.to_vec()
    };
    WireResponse {
        status,
        headers,
        body,
    }
}

/// Undo `transfer-encoding: chunked` framing.
pub(crate) fn dechunk(mut input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let eol = input
            .windows(2)
            .position(|w| w == b"\r\n")
            .expect("chunk has a size line");
        let size_hex = std::str::from_utf8(&input[..eol]).expect("chunk size is ASCII");
        let size = usize::from_str_radix(size_hex.trim(), 16).expect("chunk size parses");
        input = &input[eol + 2..];
        if size == 0 {
            break;
        }
        out.extend_from_slice(&input[..size]);
        input = &input[size + 2..];
    }
    out
}

/// First value of `name` in a raw header map (the name is matched
/// lowercased, exactly like [`WireResponse::header`]).
pub(crate) fn header_first(headers: &HashMap<String, Vec<String>>, name: &str) -> Option<String> {
    headers
        .get(&name.to_ascii_lowercase())
        .and_then(|values| values.first())
        .cloned()
}

/// First `name=value` pair of every `Set-Cookie`, joined as a `Cookie` header.
///
/// The wire-flavored adapter over the shared
/// [`super::helpers::cookie_jar_from`] core — the same core the in-process
/// `super::helpers::cookie_jar` uses, so the two transports cannot drift.
/// (Same name, different input type: a suite importing both must alias one.)
pub(crate) fn cookie_jar(headers: &HashMap<String, Vec<String>>) -> String {
    super::helpers::cookie_jar_from(
        headers
            .get("set-cookie")
            .into_iter()
            .flatten()
            .map(String::as_str),
    )
}

/// Assert the response is an RFC9457 problem+json error for `slug`.
///
/// Delegates to [`super::helpers::assert_problem_fields`] — the same core the
/// in-process suites assert with.
pub(crate) fn assert_problem(response: &WireResponse, body: &Value, slug: &str, what: &str) {
    let content_type = response
        .header("content-type")
        .unwrap_or_else(|| panic!("{what}: error must set Content-Type"));
    assert_problem_fields(content_type, body, response.status, slug, what);
}

/// Bind the loopback listener a black-box suite drives, and return it with
/// the address the suite must actually connect to.
///
/// The port is OS-assigned (ephemeral) unless `E2E_BASE_URL` pins one. That
/// is the whole point: the two black-box suites run in PARALLEL inside the
/// same test binary, so a fixed default port made one of them fail with
/// `AddrInUse` — and a dev server already holding `:3001` could break the
/// suite too. An ephemeral port removes the whole class of conflict.
///
/// The returned `SocketAddr` is the source of truth: the OS may hand out a
/// different port than requested, so never re-derive the address from the
/// requested port.
pub(crate) async fn bind_blackbox() -> (tokio::net::TcpListener, SocketAddr) {
    // Port 0 = "let the OS pick a free ephemeral port".
    let port = pinned_port().unwrap_or(0);
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .expect("black-box server binds a loopback port");
    let addr = listener
        .local_addr()
        .expect("black-box listener has a local address");
    (listener, addr)
}

/// The port pinned by `E2E_BASE_URL`, if it names one.
fn pinned_port() -> Option<u16> {
    let url = super::helpers::e2e_base_url()?;
    url.rsplit(':')
        .next()
        .and_then(|port| port.trim_end_matches('/').parse().ok())
}
