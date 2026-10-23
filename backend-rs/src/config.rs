//! Boot settings (dotenvy + envy). Full key set per `10-architecture.md` Config.
//!
//! Precedence: real environment > `.env` > defaults. [`Settings::load`]
//! validates everything fail-fast (secret length in bytes, port range,
//! `APP_ENV` whitelist, `HASH_COST` prod floor, `DATABASE_URL` charset +
//! database-name identifier rule, CORS allowlist shape) and never logs
//! secrets — [`Settings`] implements a redacting
//! `fmt::Debug`.
//!
//! **The runtime-mode key is `APP_ENV`, not `NODE_ENV`** (`54` A1). `NODE_ENV`
//! is a Node.js/npm convention with no meaning in a Rust binary. Boot
//! **fails** when `NODE_ENV` is present in the environment (see
//! [`ConfigError::LegacyNodeEnv`]): `APP_ENV` defaults to `development`, so a
//! stale deployment that still exports `NODE_ENV=production` and has not yet
//! added `APP_ENV` would otherwise boot as `development` — silently losing
//! `Secure` cookies, the `__Host-` prefix, prod HSTS and the deny-all CORS
//! default. A rename must never fail open.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::db;

/// Minimum `SESSION_SECRET` length in **bytes** (not chars).
pub const MIN_SESSION_SECRET_BYTES: usize = 32;

/// `DATABASE_URL` must always carry this query flag (see
/// `31-live-unicode-migration.md`); the derived URL adds it, an explicit
/// `DATABASE_URL` is validated to contain it.
pub const REQUIRED_DB_CHARSET: &str = "charset=utf8mb4";

/// `DATABASE_URL` must also carry this **collation** pin.
///
/// sqlx 0.8 always sent `SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci`;
/// **sqlx 0.9 sends only `SET NAMES utf8mb4`** and lets the *server* pick
/// the collation — which on MySQL 8 is `utf8mb4_0900_ai_ci`, a **different
/// collation with different comparison rules** from this project's schema
/// (`utf8mb4_unicode_ci`). Without the pin the driver version would silently
/// change the connection's collation, so the pin makes the two versions
/// byte-identical on this axis and pins the value regardless of the
/// server's default.
///
/// The **database's** own collation still matters for the `UNIQUE` indexes
/// (an index uses its COLUMN's collation, not the session's) — that is why
/// every Rust database is created with an explicit
/// `COLLATE utf8mb4_unicode_ci` (see `docs/data-migration-live.md`).
pub const REQUIRED_DB_COLLATION: &str = "collation=utf8mb4_unicode_ci";

/// Dev-only default CORS allowlist (config-driven for prod).
pub const DEFAULT_CORS_ORIGIN: &str = "http://localhost:5173";

/// The retired Node.js runtime-mode key. Its PRESENCE in the environment
/// fails the boot (see [`ConfigError::LegacyNodeEnv`]) — `APP_ENV` is the
/// key this crate reads.
pub const LEGACY_ENV_KEY: &str = "NODE_ENV";

/// Session cookie base name (dev/test). Production prefixes it with
/// [`HOST_COOKIE_PREFIX`].
///
/// `connect.sid` was **express-session's default**, copied verbatim from the
/// Nest source (`backend/src/main.ts:39`). A Rust service must not name its
/// session cookie after an Express middleware — the name misidentifies the
/// implementation to every operator, browser extension and log scraper that
/// keys on cookie names. (`54` A2.)
pub const SESSION_COOKIE_NAME: &str = "kanpad.sid";

/// RFC 6265bis `__Host-` prefix — **production only**.
///
/// The prefix is load-bearing hardening: it makes the cookie **host-only**
/// (no `Domain` ⇒ no subdomain fixation), it requires `Secure` (⇒ no
/// plaintext transmission) and it requires `Path=/`. It stays
/// environment-conditional because `__Host-` mandates `Secure`, which
/// browsers transmit only over HTTPS — the plain-HTTP staging box in
/// `docs/data-migration-live.md` therefore needs the un-prefixed name.
pub const HOST_COOKIE_PREFIX: &str = "__Host-";

/// Runtime mode read from the `APP_ENV` env var (`development|test|
/// production`). The wire format is UNCHANGED: the same three lowercase
/// strings are accepted (and rejected with the same message) as before —
/// only the in-memory representation is now a type, so an unknown value
/// cannot exist past deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppEnv {
    /// Local development (the `APP_ENV` default when unset).
    #[default]
    Development,
    /// Automated tests.
    Test,
    /// Production hardening (Secure cookies, `__Host-` prefix, HSTS, CORS
    /// deny-all default).
    Production,
}

impl AppEnv {
    /// The canonical lowercase spelling — the value `APP_ENV` accepts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        }
    }
}

/// Renders the canonical lowercase spelling (`"production"`, …).
impl fmt::Display for AppEnv {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Parses the canonical lowercase spelling; anything else is rejected with
/// the SAME message the old string-whitelist `validate()` produced, so the
/// boot error still names `APP_ENV` and its allowed set.
impl std::str::FromStr for AppEnv {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "APP_ENV must be one of development|test|production, got {other:?}"
            )),
        }
    }
}

/// Deserializes a string and runs it through [`AppEnv::from_str`] — this is
/// what keeps the BOOT error (via `envy`) naming `APP_ENV` and the allowed
/// set, exactly as the deleted whitelist block did.
impl<'de> serde::Deserialize<'de> for AppEnv {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

/// Application settings. Precedence: real environment > `.env` > defaults.
#[derive(Clone, Deserialize)]
pub struct Settings {
    /// One of `development|test|production` (`APP_ENV`); defaults to
    /// [`AppEnv::Development`] when the variable is unset.
    #[serde(default)]
    pub app_env: AppEnv,
    /// Serve port. `0` is rejected.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Argon2id `t` cost. Tests pin `t=1` for speed; default `2`.
    #[serde(default = "default_hash_cost")]
    pub hash_cost: u32,
    /// Cookie-signing secret (required, `≥32` bytes). Rotation invalidates
    /// all sessions. Never logged (see the redacting `Debug` impl).
    pub session_secret: SecretString,
    /// Explicit full DB URL (wins over the `DB_*` parts below).
    #[serde(default)]
    pub database_url: Option<String>,
    /// DB parts (mirror `backend/.env.example`); used only when
    /// `DATABASE_URL` is unset.
    #[serde(default = "default_db_host")]
    pub db_host: String,
    /// DB parts.
    #[serde(default = "default_db_port")]
    pub db_port: u16,
    /// DB parts.
    #[serde(default = "default_db_username")]
    pub db_username: String,
    /// DB parts (never logged).
    #[serde(default = "default_db_password")]
    pub db_password: SecretString,
    /// DB parts.
    #[serde(default = "default_db_name")]
    pub db_name: String,
    /// Gates `POST /api/v1/auth/register` (same semantics as Nest).
    #[serde(default = "default_true")]
    pub registration_enabled: bool,
    /// Governor global limit per IP per minute (`/api/*` only, T30).
    #[serde(default = "default_throttle_global")]
    pub throttle_global_per_min: u32,
    /// Governor auth limit per IP per minute (login/register, T30).
    #[serde(default = "default_throttle_auth")]
    pub throttle_auth_per_min: u32,
    /// Comma-separated CORS allowlist. Dev default is `DEFAULT_CORS_ORIGIN`;
    /// empty in production = deny-all (boot failure, fail closed).
    #[serde(default = "default_cors_origins")]
    pub cors_origins: String,
    /// Comma-separated CIDRs allowed to supply `X-Forwarded-For`. Empty
    /// default = never trust it (spoof defense; honored in T30).
    #[serde(default)]
    pub trusted_proxies: String,
}

fn default_port() -> u16 {
    3001
}

fn default_hash_cost() -> u32 {
    2
}

fn default_db_host() -> String {
    "localhost".to_string()
}

fn default_db_port() -> u16 {
    3306
}

fn default_db_username() -> String {
    "kanpad".to_string()
}

fn default_db_password() -> SecretString {
    SecretString::new(String::new().into_boxed_str())
}

fn default_db_name() -> String {
    "kanpad".to_string()
}

fn default_true() -> bool {
    true
}

fn default_throttle_global() -> u32 {
    120
}

fn default_throttle_auth() -> u32 {
    5
}

fn default_cors_origins() -> String {
    DEFAULT_CORS_ORIGIN.to_string()
}

/// Redacting `Debug`: secrets and DB credentials render as `[redacted]`.
/// New secret-bearing fields MUST be added here redacted, never verbatim.
impl fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("app_env", &self.app_env)
            .field("port", &self.port)
            .field("hash_cost", &self.hash_cost)
            .field("session_secret", &"[redacted]")
            .field(
                "database_url",
                &self.database_url.as_ref().map(|_| "[redacted]"),
            )
            .field("db_host", &self.db_host)
            .field("db_port", &self.db_port)
            .field("db_username", &self.db_username)
            .field("db_password", &"[redacted]")
            .field("db_name", &self.db_name)
            .field("registration_enabled", &self.registration_enabled)
            .field("throttle_global_per_min", &self.throttle_global_per_min)
            .field("throttle_auth_per_min", &self.throttle_auth_per_min)
            .field("cors_origins", &self.cors_origins)
            .field("trusted_proxies", &self.trusted_proxies)
            .finish()
    }
}

/// Test/dev convenience defaults (never used at boot — `main` always calls
/// [`Settings::load`]). Keeps integration-test construction one line:
/// `Settings { app_env: AppEnv::Test, ..Default::default() }`.
impl Default for Settings {
    fn default() -> Self {
        Self {
            app_env: AppEnv::Test,
            port: 3001,
            hash_cost: 1,
            session_secret: SecretString::new(
                "test-session-secret-that-is-longer-than-32-bytes".into(),
            ),
            database_url: None,
            db_host: default_db_host(),
            db_port: default_db_port(),
            db_username: default_db_username(),
            db_password: default_db_password(),
            db_name: default_db_name(),
            registration_enabled: true,
            throttle_global_per_min: default_throttle_global(),
            throttle_auth_per_min: default_throttle_auth(),
            cors_origins: default_cors_origins(),
            trusted_proxies: String::new(),
        }
    }
}

/// Boot-time configuration failure.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Env deserialization failure.
    #[error("invalid configuration: {0}")]
    Env(#[from] envy::Error),
    /// Validation failure with an actionable message.
    #[error("{0}")]
    Invalid(String),
    /// The Node.js-only `NODE_ENV` was found in the environment.
    ///
    /// `NODE_ENV` is a Node.js/npm convention; `backend-rs` is a Rust
    /// service and uses [`APP_ENV`](Settings::app_env). Boot fails rather
    /// than ignoring it, because `APP_ENV` defaults to `development` — a
    /// stale deployment would otherwise boot as `development` and silently
    /// lose `Secure` cookies, the `__Host-` prefix, prod HSTS and the
    /// deny-all CORS default.
    #[error(
        "configuration error: NODE_ENV is not used by backend-rs (this is a Rust service, not a \
         Node one) — set APP_ENV=development|test|production instead"
    )]
    LegacyNodeEnv,
}

impl Settings {
    /// Load once at boot: `.env` (if present), then environment, then validate.
    pub fn load() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();
        Settings::load_from_env()
    }

    /// Load from the process environment **only** — no `.env` lookup.
    ///
    /// `dotenvy::dotenv()` never overrides an existing variable, so it can
    /// only *add* keys. A developer's local `.env` therefore silently
    /// re-supplies a key a caller deliberately unset (e.g. `SESSION_SECRET`),
    /// which makes a "this must fail to boot" assertion pass for the wrong
    /// reason. Boot keeps using [`Settings::load`]; tests that assert on a
    /// missing/invalid key use this so ambient `.env` cannot leak in.
    ///
    /// **The legacy-`NODE_ENV` check runs FIRST**, before `envy` reads
    /// `APP_ENV`: if `NODE_ENV` were merely ignored, `APP_ENV`'s
    /// `development` default would silently apply to a deployment that meant
    /// `production` (`54` A1 fail-fast rule).
    pub fn load_from_env() -> Result<Self, ConfigError> {
        if std::env::var_os(LEGACY_ENV_KEY).is_some() {
            return Err(ConfigError::LegacyNodeEnv);
        }
        let settings: Settings = envy::from_env()?;
        settings.validate()?;
        Ok(settings)
    }

    /// Load from an explicit key/value map — the **test seam**.
    ///
    /// Edition 2024 made `std::env::set_var` / `remove_var` **UNSAFE**
    /// (they race with any concurrent environment access), and this crate
    /// sets `unsafe_code = "forbid"` — so the config tests must NOT mutate
    /// the process environment. They build a map and load from it instead.
    /// The deserialize + validate path is identical; only the SOURCE
    /// differs. A side benefit: the tests no longer need the `ENV_LOCK`
    /// mutex (they were serialized for no reason) and are now
    /// parallel-safe.
    #[cfg(test)]
    pub(crate) fn from_pairs(pairs: &[(&str, &str)]) -> Result<Self, ConfigError> {
        if pairs.iter().any(|(key, _)| *key == LEGACY_ENV_KEY) {
            return Err(ConfigError::LegacyNodeEnv);
        }
        let settings: Settings = envy::from_iter(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string())),
        )?;
        settings.validate()?;
        Ok(settings)
    }

    /// Effective DB URL: explicit `DATABASE_URL` wins, otherwise built from
    /// the `DB_*` parts with `?charset=utf8mb4` always appended. Built in
    /// this one function only (pool creation calls it once).
    pub fn database_url(&self) -> String {
        if let Some(explicit) = self.database_url.as_deref() {
            return explicit.to_string();
        }
        format!(
            "mysql://{}:{}@{}:{}/{}?charset={}&collation={}",
            self.db_username,
            self.db_password.expose_secret(),
            self.db_host,
            self.db_port,
            self.db_name,
            "utf8mb4",
            "utf8mb4_unicode_ci"
        )
    }

    /// Parsed CORS allowlist (trimmed, empties dropped). The wildcard `"*"`
    /// is rejected at `Settings::validate` because the stack always sends
    /// `allow_credentials(true)` (browsers reject `*` + credentials).
    pub fn cors_origin_list(&self) -> Vec<String> {
        self.cors_origins
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// The **canonical** CORS allowlist — the SINGLE source of truth for
    /// `CorsLayer::allow_origin`, `CsrfLayer::add_trusted_origin` AND the
    /// CSP's `connect-src` (three consumers, one list).
    ///
    /// Every entry is rendered in the exact form a **browser** sends in an
    /// `Origin` header, because both `CorsLayer` and `CsrfLayer` match
    /// origins **byte-for-byte** (`tower-http`'s own docs: an entry that is
    /// not in canonical browser form *"will silently never match"*).
    /// [`Settings::validate`] rejects a non-canonical entry at boot, so in a
    /// real deployment this list is already canonical; the normalization here
    /// keeps a hand-built [`Settings`] (tests) from silently degrading.
    ///
    /// An entry that cannot be parsed as an origin at all is **dropped** —
    /// dropping is fail-CLOSED (the origin is then not trusted and not
    /// allowed), never fail-open.
    pub fn canonical_cors_origins(&self) -> Vec<String> {
        self.cors_origin_list()
            .iter()
            .filter_map(|entry| canonical_origin(entry).ok())
            .collect()
    }

    /// Parsed trusted-proxy list. Empty default = never trust
    /// `X-Forwarded-For` (direct-to-`:3001` spoof defense).
    pub fn trusted_proxy_list(&self) -> Vec<String> {
        self.trusted_proxies
            .split(',')
            .map(str::trim)
            .filter(|proxy| !proxy.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Production predicate (secure cookies, `__Host-` prefix, HSTS, CORS
    /// deny-all default).
    pub fn is_production(&self) -> bool {
        self.app_env == AppEnv::Production
    }

    /// Session cookie name by env (`10-architecture.md` Sessions/Cookie):
    /// `kanpad.sid` in dev/test, `__Host-kanpad.sid` in production (where
    /// it MUST pair with `Secure + Path=/` and no `Domain` — enforced by the
    /// `SessionManagerLayer` wiring in `lib.rs`, asserted per env in T11/T54).
    ///
    /// The name is **app-scoped, not framework-scoped**: `connect.sid` was
    /// express-session's default and is retired (`54` A2).
    pub fn session_cookie_name(&self) -> String {
        if self.is_production() {
            format!("{HOST_COOKIE_PREFIX}{SESSION_COOKIE_NAME}")
        } else {
            SESSION_COOKIE_NAME.to_string()
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::Invalid("PORT must be non-zero".to_string()));
        }
        // The `APP_ENV` value whitelist is gone (T58): an unparseable value
        // can no longer reach `Settings` — `AppEnv`'s `Deserialize` rejects
        // it at deserialization with the same message the old block built.
        if self.app_env == AppEnv::Production && self.hash_cost < 2 {
            return Err(ConfigError::Invalid(
                "HASH_COST must be >= 2 in production".to_string(),
            ));
        }
        // `str::len` is byte length; the explicit byte framing is intentional
        // (spec: secret floor is 32 *bytes*, not chars).
        if self.session_secret.expose_secret().len() < MIN_SESSION_SECRET_BYTES {
            return Err(ConfigError::Invalid(format!(
                "SESSION_SECRET must be at least {MIN_SESSION_SECRET_BYTES} bytes"
            )));
        }
        if !self.database_url().contains(REQUIRED_DB_CHARSET) {
            return Err(ConfigError::Invalid(
                "DATABASE_URL must include ?charset=utf8mb4".to_string(),
            ));
        }
        // The collation pin is REQUIRED, not decorative: sqlx 0.9 sends only
        // `SET NAMES utf8mb4` and lets the server pick (MySQL 8's default is
        // `utf8mb4_0900_ai_ci` — a different collation from this schema's
        // `utf8mb4_unicode_ci`). Pinning it makes the driver version
        // irrelevant on this axis.
        if !self.database_url().contains(REQUIRED_DB_COLLATION) {
            return Err(ConfigError::Invalid(format!(
                "DATABASE_URL must include &{REQUIRED_DB_COLLATION} — sqlx 0.9 lets the server \
                 pick the collation otherwise (MySQL 8 defaults to utf8mb4_0900_ai_ci, not this \
                 schema's utf8mb4_unicode_ci)"
            )));
        }
        // Screen the database name against `db::is_valid_db_name`: it must
        // be usable as a plain identifier (a leading digit, or `-`/`.`/space,
        // is a legal MySQL name that operator tooling does not quote). A bad
        // DB_NAME must fail here as a clean config error, not as a surprise
        // at the first query or in a `mysql -D`. Never echoes credentials.
        let db_name = self
            .database_url()
            .parse::<sqlx::mysql::MySqlConnectOptions>()
            .ok()
            .and_then(|options| options.get_database().map(str::to_string))
            .unwrap_or_else(|| self.db_name.clone());
        if !db::is_valid_db_name(&db_name) {
            return Err(ConfigError::Invalid(format!(
                "database name {db_name:?} is not a valid MySQL identifier: it must start with a \
                 letter or underscore and contain only letters, digits, `_`, or `$`"
            )));
        }
        let origins = self.cors_origin_list();
        if origins.iter().any(|origin| origin == "*") {
            return Err(ConfigError::Invalid(
                "CORS_ORIGINS must not contain '*' together with credentials (browsers reject Any + Access-Control-Allow-Credentials)".to_string(),
            ));
        }
        for origin in &origins {
            // CANONICAL FORM (T56). Both `CorsLayer` and `CsrfLayer` match an
            // origin BYTE-FOR-BYTE, and `tower-http`'s own docs warn that an
            // entry which is not in canonical browser form "will silently
            // never match" — `http://localhost:5173/` (trailing slash),
            // `HTTP://LOCALHOST:5173` (uppercase) and
            // `http://example.com:443` (explicit default port) would each
            // be accepted by the layer and then never match a browser's
            // `Origin` header ⇒ every write 403s with no clue why.
            //
            // So the entry is parsed into canonical form and the RAW entry is
            // compared against it: a difference is a BOOT ERROR naming the
            // offending entry (never a silent non-match).
            match canonical_origin(origin) {
                Ok(canonical) if canonical == *origin => {}
                Ok(canonical) => {
                    return Err(ConfigError::Invalid(format!(
                        "CORS_ORIGINS entry {origin:?} is not in canonical form (a browser sends \
                         {canonical:?}) — write it exactly as a browser sends it: lowercase scheme \
                         and host, no trailing slash, no explicit default port, no \
                         path/query/fragment"
                    )));
                }
                Err(reason) => {
                    return Err(ConfigError::Invalid(format!(
                        "CORS_ORIGINS entry {origin:?} is not a valid origin: {reason}"
                    )));
                }
            }
        }
        if self.is_production() && origins.is_empty() {
            return Err(ConfigError::Invalid(
                "CORS_ORIGINS must not be empty in production (fail closed: deny-all)".to_string(),
            ));
        }
        Ok(())
    }
}

/// Canonical form of a browser `Origin` header value.
///
/// A browser sends an origin as `scheme://host[:port]` with
///
/// * a **lowercase** scheme and host (IDN hosts in punycode),
/// * the scheme's **default port omitted** (`:80` for http, `:443` for
///   https — a browser never sends them),
/// * **no** trailing slash, path, query or fragment.
///
/// This function returns exactly that rendering for a well-formed entry, or
/// an actionable reason why the entry cannot be one. It is the single
/// definition of "canonical" for the whole service: the CORS allowlist, the
/// CSRF trusted-origin set and the CSP's `connect-src` are all derived from
/// it, so they can never disagree.
///
/// The scheme's default port is **stripped** rather than rejected here: the
/// caller ([`Settings::validate`]) compares the canonical rendering with the
/// raw entry and fails the boot when they differ — that is what turns a
/// silent non-match into an actionable boot error.
pub fn canonical_origin(entry: &str) -> Result<String, String> {
    let Some((scheme, rest)) = entry.split_once("://") else {
        return Err("it must be scheme://host[:port]".to_string());
    };
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return Err("the scheme must be http or https".to_string());
    }
    if rest.is_empty() {
        return Err("the host is missing".to_string());
    }
    if rest.contains('/') {
        return Err("a path (or a trailing slash) is not allowed".to_string());
    }
    if rest.contains('?') {
        return Err("a query string is not allowed".to_string());
    }
    if rest.contains('#') {
        return Err("a fragment is not allowed".to_string());
    }
    let (host, port) = split_host_port(rest)?;
    if host.is_empty() {
        return Err("the host is missing".to_string());
    }
    if !host.is_ascii() {
        return Err(
            "a non-ASCII host must be supplied in punycode (e.g. xn--exmple-cua.com)".to_string(),
        );
    }
    // A browser lowercases the host; the canonical rendering does too (the
    // caller then sees a difference and fails the boot).
    let host = host.to_ascii_lowercase();
    // A default port is never sent by a browser → strip it (the caller then
    // sees a difference and fails the boot).
    let default_port = match scheme.as_str() {
        "http" => 80,
        _ => 443,
    };
    let port = match port {
        Some(port) if port == default_port => None,
        other => other,
    };
    Ok(match port {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    })
}

/// Split `host[:port]` (also handling an IPv6 literal `[::1]:8080`).
fn split_host_port(rest: &str) -> Result<(String, Option<u16>), String> {
    if let Some(close) = rest.find(']') {
        let host = &rest[..=close];
        let after = &rest[close + 1..];
        let port = match after.strip_prefix(':') {
            Some(port) => Some(parse_port(port)?),
            None if after.is_empty() => None,
            None => return Err("the port must be separated by a colon".to_string()),
        };
        return Ok((host.to_string(), port));
    }
    match rest.split_once(':') {
        Some((host, port)) => Ok((host.to_string(), Some(parse_port(port)?))),
        None => Ok((rest.to_string(), None)),
    }
}

/// Parse a port: decimal digits only, no leading zeros, `1..=65535`.
fn parse_port(raw: &str) -> Result<u16, String> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("the port must be a number".to_string());
    }
    let port: u16 = raw
        .parse()
        .map_err(|_| "the port must be between 1 and 65535".to_string())?;
    if port == 0 {
        return Err("the port must be between 1 and 65535".to_string());
    }
    if port.to_string() != raw {
        return Err("the port must not have leading zeros".to_string());
    }
    Ok(port)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    // NOTE: the tests build a KEY/VALUE MAP and load from it
    // ([`Settings::from_pairs`]) — they never mutate the process
    // environment: edition 2024 made `std::env::set_var` unsafe, and this
    // crate forbids `unsafe`. The matrix below therefore sets every key
    // EXPLICITLY (so ambient process env — CI's `DATABASE_URL`, a local
    // `PORT` — cannot leak in), and the legacy Node key is deliberately
    // ABSENT (a map that contains it must fail the load).

    const BASE_VARS: &[(&str, &str)] = &[
        ("APP_ENV", "test"),
        ("PORT", "3001"),
        ("HASH_COST", "1"),
        (
            "SESSION_SECRET",
            "test-session-secret-that-is-longer-than-32-bytes",
        ),
        (
            "DATABASE_URL",
            "mysql://kanpad:pw@127.0.0.1:3306/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci",
        ),
        ("REGISTRATION_ENABLED", "true"),
        ("THROTTLE_GLOBAL_PER_MIN", "120"),
        ("THROTTLE_AUTH_PER_MIN", "5"),
        ("CORS_ORIGINS", "http://localhost:5173"),
        ("TRUSTED_PROXIES", ""),
    ];

    fn base_vars() -> Vec<(&'static str, &'static str)> {
        BASE_VARS.to_vec()
    }

    /// Env matrix driving `DB_NAME` instead of an explicit `DATABASE_URL`
    /// (an explicit URL wins over the parts, so it must be removed).
    fn with_db_name(name: &'static str) -> Vec<(&'static str, &'static str)> {
        let mut vars = base_vars();
        vars.retain(|(key, _)| *key != "DATABASE_URL");
        vars.push(("DB_NAME", name));
        vars
    }

    fn with_overrides(
        overrides: &[(&'static str, &'static str)],
    ) -> Vec<(&'static str, &'static str)> {
        let mut vars = base_vars();
        for (key, value) in overrides {
            if let Some(slot) = vars.iter_mut().find(|(name, _)| name == key) {
                slot.1 = value;
            } else {
                vars.push((key, value));
            }
        }
        vars
    }

    #[test]
    fn hash_cost_floor() {
        // Production with HASH_COST=1 must fail boot.
        {
            let vars = with_overrides(&[("APP_ENV", "production"), ("HASH_COST", "1")]);
            let err =
                Settings::from_pairs(&vars).expect_err("production HASH_COST=1 must fail boot");
            assert!(
                err.to_string().contains("HASH_COST"),
                "unexpected error: {err}"
            );
        }
        // Tests pin t=1 and must load outside production.
        {
            let vars = with_overrides(&[("APP_ENV", "test"), ("HASH_COST", "1")]);
            let settings = Settings::from_pairs(&vars).expect("test HASH_COST=1 must load");
            assert_eq!(settings.hash_cost, 1);
        }
    }

    #[test]
    fn missing_session_secret_fails_boot() {
        let mut vars = base_vars();
        vars.retain(|(name, _)| *name != "SESSION_SECRET");
        let err = Settings::from_pairs(&vars).expect_err("missing SESSION_SECRET must fail boot");
        assert!(
            err.to_string().contains("session_secret"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn short_session_secret_fails_boot() {
        for secret in ["short", "0123456789abcdef0123456789abcde"] {
            assert!(secret.len() < MIN_SESSION_SECRET_BYTES);
            let vars = with_overrides(&[("SESSION_SECRET", secret)]);
            let err = Settings::from_pairs(&vars).expect_err("short SESSION_SECRET must fail boot");
            assert!(
                err.to_string().contains("SESSION_SECRET"),
                "unexpected error: {err}"
            );
        }
        // Exactly 32 bytes loads.
        let vars = with_overrides(&[("SESSION_SECRET", "0123456789abcdef0123456789abcdef")]);
        Settings::from_pairs(&vars).expect("32-byte SESSION_SECRET must load");
    }

    /// AC (T54, `54` A1): the legacy Node.js `NODE_ENV` in the environment
    /// must **fail the boot** — never be silently ignored. `APP_ENV` defaults
    /// to `development`, so ignoring `NODE_ENV=production` would silently
    /// drop `Secure` cookies, the `__Host-` prefix, prod HSTS and the
    /// deny-all CORS default. The error must name `APP_ENV` so the operator
    /// knows what to set.
    #[test]
    fn legacy_node_env_fails_boot_and_names_app_env() {
        for legacy in ["production", "development", ""] {
            let mut vars = with_overrides(&[("NODE_ENV", legacy)]);
            // `APP_ENV` deliberately left unset: the point is that the
            // legacy key alone must abort the boot.
            vars.retain(|(name, _)| *name != "APP_ENV");
            let err = Settings::from_pairs(&vars)
                .expect_err("NODE_ENV in the environment must fail boot");
            let message = err.to_string();
            assert!(
                message.contains("NODE_ENV"),
                "the error must name the offending key: {message}"
            );
            assert!(
                message.contains("APP_ENV"),
                "the error must name the replacement key: {message}"
            );
        }
        // The legacy key absent + APP_ENV set -> boots.
        let vars = with_overrides(&[("APP_ENV", "test")]);
        Settings::from_pairs(&vars).expect("APP_ENV alone must load");
    }

    /// AC (T54): the session cookie name is **app-scoped** — the Express
    /// default `connect.sid` must never come back, in any environment.
    #[test]
    fn session_cookie_never_uses_the_express_default() {
        for env in ["development", "test", "production"] {
            let vars = if env == "production" {
                with_overrides(&[("APP_ENV", env), ("HASH_COST", "2")])
            } else {
                with_overrides(&[("APP_ENV", env)])
            };
            let settings = Settings::from_pairs(&vars).expect("env must load");
            let name = settings.session_cookie_name();
            assert!(
                !name.contains("connect.sid"),
                "env {env} must not emit the Express cookie name: {name}"
            );
            assert!(
                name.ends_with(SESSION_COOKIE_NAME),
                "env {env} must use the app-scoped cookie name: {name}"
            );
        }
    }

    #[test]
    fn app_env_whitelist_and_port_rejects() {
        for bad_env in ["prod", "Production", "staging", ""] {
            let vars = with_overrides(&[("APP_ENV", bad_env)]);
            let err = Settings::from_pairs(&vars).expect_err("bad APP_ENV must fail boot");
            assert!(
                err.to_string().contains("APP_ENV"),
                "unexpected error for {bad_env:?}: {err}"
            );
        }
        for good_env in ["development", "test"] {
            let vars = with_overrides(&[("APP_ENV", good_env)]);
            Settings::from_pairs(&vars).expect("whitelisted APP_ENV must load");
        }
        // Production needs HASH_COST>=2 to get past the cost floor.
        {
            let vars = with_overrides(&[("APP_ENV", "production"), ("HASH_COST", "2")]);
            Settings::from_pairs(&vars).expect("production APP_ENV must load");
        }
        let vars = with_overrides(&[("PORT", "0")]);
        let err = Settings::from_pairs(&vars).expect_err("PORT=0 must fail boot");
        assert!(err.to_string().contains("PORT"), "unexpected error: {err}");
    }

    /// AC: a database name that is not usable as a plain identifier (leading
    /// digit, hyphen, dot, space) is a clean config error — never a boot
    /// panic (see `db::is_valid_db_name`).
    #[test]
    fn rejects_database_names_the_session_store_would_refuse() {
        for (name, why) in [
            ("123kanpad", "leading digit"),
            ("kan-pad", "hyphen"),
            ("kan.pad", "dot"),
            ("kan pad", "space"),
        ] {
            let vars = with_db_name(name);

            let err = Settings::from_pairs(&vars).expect_err("invalid DB_NAME must fail boot");
            let message = err.to_string();
            assert!(
                message.contains("not a valid MySQL identifier"),
                "{why} ({name:?}): unexpected error {message}"
            );
        }
    }

    /// AC: the legal-identifier happy path still boots — guards the check
    /// above against over-rejecting the names sqlx actually creates.
    #[test]
    fn accepts_sqlx_test_style_database_names() {
        for name in ["kanpad", "_sqlx_test_a1b2c3", "kanpad_prod_1"] {
            let vars = with_db_name(name);
            Settings::from_pairs(&vars)
                .unwrap_or_else(|err| panic!("{name:?} must be accepted: {err}"));
        }
    }

    #[test]
    fn database_url_charset_and_derivation() {
        // Explicit URL without the charset flag is rejected.
        {
            let vars = with_overrides(&[("DATABASE_URL", "mysql://u:p@h:3306/db")]);
            let err =
                Settings::from_pairs(&vars).expect_err("charset-less DATABASE_URL must fail boot");
            assert!(
                err.to_string().contains("charset=utf8mb4"),
                "unexpected error: {err}"
            );
        }
        // Explicit URL with the charset but WITHOUT the collation pin is
        // rejected too: sqlx 0.9 lets the server pick the collation, and
        // MySQL 8's default (`utf8mb4_0900_ai_ci`) is NOT this schema's
        // (`utf8mb4_unicode_ci`).
        {
            let vars = with_overrides(&[("DATABASE_URL", "mysql://u:p@h:3306/db?charset=utf8mb4")]);
            let err = Settings::from_pairs(&vars)
                .expect_err("a collation-less DATABASE_URL must fail boot");
            assert!(
                err.to_string().contains("collation=utf8mb4_unicode_ci"),
                "unexpected error: {err}"
            );
        }
        // Without DATABASE_URL the URL is derived from parts (charset always).
        {
            let mut vars = base_vars();
            vars.retain(|(name, _)| *name != "DATABASE_URL");
            vars.extend_from_slice(&[
                ("DB_HOST", "db.internal"),
                ("DB_PORT", "3307"),
                ("DB_USERNAME", "app"),
                ("DB_PASSWORD", "s3cret"),
                ("DB_NAME", "kanpad"),
            ]);
            let settings = Settings::from_pairs(&vars).expect("parts-derived config must load");
            assert_eq!(
                settings.database_url(),
                "mysql://app:s3cret@db.internal:3307/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci",
                "the DERIVED url carries the collation pin — compose passes only the DB_* parts"
            );
        }
        // Explicit DATABASE_URL wins over parts.
        {
            let vars = with_overrides(&[
                (
                    "DATABASE_URL",
                    "mysql://x:y@h:1/z?charset=utf8mb4&collation=utf8mb4_unicode_ci",
                ),
                ("DB_HOST", "ignored"),
            ]);
            let settings = Settings::from_pairs(&vars).expect("explicit DATABASE_URL must load");
            assert_eq!(
                settings.database_url(),
                "mysql://x:y@h:1/z?charset=utf8mb4&collation=utf8mb4_unicode_ci"
            );
        }
    }

    #[test]
    fn cors_any_with_credentials_fails_and_prod_empty_fails_closed() {
        for origins in ["*", "http://localhost:5173, *"] {
            let vars = with_overrides(&[("CORS_ORIGINS", origins)]);
            let err = Settings::from_pairs(&vars).expect_err("wildcard CORS must fail boot");
            assert!(
                err.to_string().contains("CORS_ORIGINS"),
                "unexpected error: {err}"
            );
        }
        // Prod + empty allowlist fails closed (HASH_COST=2 to isolate the CORS gate).
        {
            let vars = with_overrides(&[
                ("APP_ENV", "production"),
                ("HASH_COST", "2"),
                ("CORS_ORIGINS", "   "),
            ]);
            let err = Settings::from_pairs(&vars).expect_err("prod empty CORS must fail boot");
            assert!(
                err.to_string().contains("CORS_ORIGINS"),
                "unexpected error: {err}"
            );
        }
        // Non-prod empty allowlist loads (deny-all at runtime).
        {
            let vars = with_overrides(&[("CORS_ORIGINS", "")]);
            let settings = Settings::from_pairs(&vars).expect("dev empty CORS must load");
            assert!(settings.cors_origin_list().is_empty());
        }
        // Malformed origins fail boot (fail fast, not silent deny).
        for origins in ["not-a-url", "localhost:5173", "ftp://files.example.com"] {
            let vars = with_overrides(&[("CORS_ORIGINS", origins)]);
            let err =
                Settings::from_pairs(&vars).expect_err("malformed CORS origin must fail boot");
            assert!(
                err.to_string().contains("CORS_ORIGINS"),
                "unexpected error for {origins:?}: {err}"
            );
        }
        // Multi-origin parsing trims and drops empties.
        {
            let vars = with_overrides(&[(
                "CORS_ORIGINS",
                " https://app.example.com ,https://admin.example.com,,",
            )]);
            let settings = Settings::from_pairs(&vars).expect("multi-origin CORS must load");
            assert_eq!(
                settings.cors_origin_list(),
                vec![
                    "https://app.example.com".to_string(),
                    "https://admin.example.com".to_string()
                ]
            );
        }
    }

    /// AC (T56): a **non-canonical** `CORS_ORIGINS` entry must fail the boot
    /// with a message naming the offending entry — never a silent non-match.
    /// `CorsLayer`/`CsrfLayer` match origins byte-for-byte, so
    /// `http://localhost:5173/`, `HTTP://LOCALHOST:5173` and
    /// `http://example.com:443` would be accepted and then never match a
    /// browser's `Origin` header ⇒ every write 403s for no visible reason.
    #[test]
    fn non_canonical_cors_origins_fail_boot_naming_the_entry() {
        for entry in [
            "http://localhost:5173/",
            "HTTP://LOCALHOST:5173",
            "https://example.com:443",
            "http://example.com:80",
            "http://localhost:5173/app",
            "http://localhost:5173?x=1",
            "http://localhost:5173#f",
            "ftp://files.example.com",
            "localhost:5173",
        ] {
            let vars = with_overrides(&[("CORS_ORIGINS", entry)]);
            let err = Settings::from_pairs(&vars)
                .expect_err("a non-canonical CORS origin must fail the boot");
            let message = err.to_string();
            assert!(
                message.contains("CORS_ORIGINS"),
                "the error must name the key: {message}"
            );
            assert!(
                message.contains(entry),
                "the error must name the offending entry {entry:?}: {message}"
            );
        }
    }

    /// The canonicalizer itself: a canonical entry is returned unchanged, and
    /// the non-canonical variants render the form a browser actually sends.
    #[test]
    fn canonical_origin_renders_the_browser_form() {
        for (entry, canonical) in [
            ("http://localhost:5173", "http://localhost:5173"),
            ("https://app.example.com", "https://app.example.com"),
            (
                "https://app.example.com:8443",
                "https://app.example.com:8443",
            ),
            ("HTTP://LOCALHOST:5173", "http://localhost:5173"),
            ("https://example.com:443", "https://example.com"),
            ("http://example.com:80", "http://example.com"),
            ("http://example.com:443", "http://example.com:443"),
            ("http://[::1]:3001", "http://[::1]:3001"),
        ] {
            assert_eq!(
                canonical_origin(entry).expect("entry must parse"),
                canonical,
                "entry: {entry}"
            );
        }
        for entry in [
            "",
            "not-a-url",
            "http://",
            "https://example.com/",
            "https://example.com/path",
            "https://example.com?a=b",
            "https://example.com#frag",
            "file:///tmp/x",
            "https://ümlaut.de",
            "https://example.com:0080",
            "https://example.com:0",
            "https://example.com:70000",
        ] {
            assert!(
                canonical_origin(entry).is_err(),
                "entry must be rejected: {entry:?}"
            );
        }
    }

    /// The canonical list is the SINGLE source of truth for the CORS layer,
    /// the CSRF trusted set and the CSP's `connect-src`.
    #[test]
    fn canonical_cors_origins_is_the_normalized_list() {
        let vars = with_overrides(&[(
            "CORS_ORIGINS",
            "https://app.example.com, http://localhost:5173",
        )]);
        let settings = Settings::from_pairs(&vars).expect("canonical origins must load");
        assert_eq!(
            settings.canonical_cors_origins(),
            vec![
                "https://app.example.com".to_string(),
                "http://localhost:5173".to_string()
            ]
        );
        // The raw list keeps the operator's spelling (the boot error names
        // the offending entry verbatim).
        assert_eq!(
            settings.cors_origin_list(),
            vec![
                "https://app.example.com".to_string(),
                "http://localhost:5173".to_string()
            ]
        );
    }

    #[test]
    fn trusted_proxies_empty_by_default() {
        let vars = base_vars();
        let settings = Settings::from_pairs(&vars).expect("defaults must load");
        assert!(settings.trusted_proxy_list().is_empty());
        assert!(settings.registration_enabled);
        assert_eq!(settings.throttle_global_per_min, 120);
        assert_eq!(settings.throttle_auth_per_min, 5);
    }

    #[test]
    fn debug_redacts_secrets() {
        let settings = Settings {
            session_secret: SecretString::new("super-secret-value-that-is-long".into()),
            database_url: Some(
                "mysql://u:exposed-pw@h:1/d?charset=utf8mb4&collation=utf8mb4_unicode_ci"
                    .to_string(),
            ),
            db_password: SecretString::new("db-password-value".into()),
            ..Default::default()
        };
        let rendered = format!("{settings:?}");
        for leaked in [
            "super-secret-value-that-is-long",
            "exposed-pw",
            "db-password-value",
        ] {
            assert!(
                !rendered.contains(leaked),
                "secret leaked in Debug: {rendered}"
            );
        }
        assert!(rendered.contains("[redacted]"), "redaction marker missing");
    }

    #[test]
    fn default_test_settings_validate() {
        let settings = Settings::default();
        settings
            .validate()
            .expect("Default test settings must validate");
        assert_eq!(settings.app_env, AppEnv::Test);
    }

    #[test]
    fn session_cookie_name_follows_env() {
        for (env, name) in [
            ("development", "kanpad.sid"),
            ("test", "kanpad.sid"),
            ("production", "__Host-kanpad.sid"),
        ] {
            // Production refuses HASH_COST<2 at boot, so pin t=2 there.
            let vars = if env == "production" {
                with_overrides(&[("APP_ENV", env), ("HASH_COST", "2")])
            } else {
                with_overrides(&[("APP_ENV", env)])
            };
            let settings = Settings::from_pairs(&vars).expect("env must load");
            let parsed = AppEnv::from_str(env).expect("loop env is a valid AppEnv");
            assert_eq!(settings.app_env, parsed, "env: {env}");
            assert_eq!(settings.session_cookie_name(), name, "env: {env}");
            assert_eq!(settings.is_production(), parsed == AppEnv::Production);
        }
        // Struct-level (no env): same rule.
        let mut settings = Settings::default();
        assert_eq!(settings.session_cookie_name(), "kanpad.sid");
        settings.app_env = AppEnv::Production;
        assert_eq!(settings.session_cookie_name(), "__Host-kanpad.sid");
    }
}
