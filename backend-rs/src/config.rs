//! Boot settings (dotenvy + envy). Full key set per `10-architecture.md` Config.
//!
//! Precedence: real environment > `.env` > defaults. [`Settings::load`]
//! validates everything fail-fast (secret length in bytes, port range,
//! `NODE_ENV` whitelist, `HASH_COST` prod floor, `DATABASE_URL` charset +
//! database-name identifier rule, CORS allowlist shape) and never logs
//! secrets — [`Settings`] implements a redacting
//! [`fmt::Debug`](std::fmt::Debug).

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

/// Dev-only default CORS allowlist (config-driven for prod).
pub const DEFAULT_CORS_ORIGIN: &str = "http://localhost:5173";

/// Application settings. Precedence: real environment > `.env` > defaults.
#[derive(Clone, Deserialize)]
pub struct Settings {
    /// One of `development|test|production`.
    #[serde(default = "default_node_env")]
    pub node_env: String,
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

fn default_node_env() -> String {
    "development".to_string()
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
            .field("node_env", &self.node_env)
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
/// `Settings { node_env: "test".into(), ..Default::default() }`.
impl Default for Settings {
    fn default() -> Self {
        Self {
            node_env: "test".to_string(),
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
    pub fn load_from_env() -> Result<Self, ConfigError> {
        let settings: Settings = envy::from_env()?;
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
            "mysql://{}:{}@{}:{}/{}?charset={}",
            self.db_username,
            self.db_password.expose_secret(),
            self.db_host,
            self.db_port,
            self.db_name,
            "utf8mb4"
        )
    }

    /// Parsed CORS allowlist (trimmed, empties dropped). The wildcard `"*"`
    /// is rejected at [`Settings::validate`] because the stack always sends
    /// `allow_credentials(true)` (browsers reject `*` + credentials).
    pub fn cors_origin_list(&self) -> Vec<String> {
        self.cors_origins
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
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
        self.node_env == "production"
    }

    /// Session cookie name by env (`10-architecture.md` Sessions/Cookie):
    /// `connect.sid` in dev/test, `__Host-connect.sid` in production (where
    /// it MUST pair with `Secure + Path=/` and no `Domain` — enforced by the
    /// `SessionManagerLayer` wiring in `lib.rs`, asserted per env in T11).
    pub fn session_cookie_name(&self) -> String {
        if self.is_production() {
            "__Host-connect.sid".to_string()
        } else {
            "connect.sid".to_string()
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        match self.node_env.as_str() {
            "development" | "test" | "production" => {}
            other => {
                return Err(ConfigError::Invalid(format!(
                    "NODE_ENV must be one of development|test|production, got {other:?}"
                )));
            }
        }
        if self.port == 0 {
            return Err(ConfigError::Invalid("PORT must be non-zero".to_string()));
        }
        if self.node_env == "production" && self.hash_cost < 2 {
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
        // The session store binds to this database via
        // `MySqlStore::with_schema_name`, whose identifier rule is STRICTER
        // than MySQL's (a leading digit, or `-`/`.`/space, is a legal schema
        // name it refuses). Screen it here so a bad DB_NAME fails as a clean
        // config error instead of panicking when the store is constructed
        // (see `db::is_valid_db_name`). Never echoes credentials.
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
            // Strict shape so `cors_layer` stays infallible: an `Origin` is
            // `scheme://host[:port]` and must parse as a header value.
            let shape_ok = (origin.starts_with("http://") || origin.starts_with("https://"))
                && axum::http::HeaderValue::from_str(origin).is_ok();
            if !shape_ok {
                return Err(ConfigError::Invalid(format!(
                    "CORS_ORIGINS entry {origin:?} must be http(s)://host[:port]"
                )));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Every `Settings` env key. Tests set the full matrix explicitly so
    /// ambient process env (CI `DATABASE_URL`, local `PORT`, …) cannot leak
    /// in; dropped guards restore the previous values.
    const SETTINGS_KEYS: &[&str] = &[
        "NODE_ENV",
        "PORT",
        "HASH_COST",
        "SESSION_SECRET",
        "DATABASE_URL",
        "DB_HOST",
        "DB_PORT",
        "DB_USERNAME",
        "DB_PASSWORD",
        "DB_NAME",
        "REGISTRATION_ENABLED",
        "THROTTLE_GLOBAL_PER_MIN",
        "THROTTLE_AUTH_PER_MIN",
        "CORS_ORIGINS",
        "TRUSTED_PROXIES",
    ];

    const BASE_VARS: &[(&str, &str)] = &[
        ("NODE_ENV", "test"),
        ("PORT", "3001"),
        ("HASH_COST", "1"),
        (
            "SESSION_SECRET",
            "test-session-secret-that-is-longer-than-32-bytes",
        ),
        (
            "DATABASE_URL",
            "mysql://kanpad:pw@127.0.0.1:3306/kanpad?charset=utf8mb4",
        ),
        ("REGISTRATION_ENABLED", "true"),
        ("THROTTLE_GLOBAL_PER_MIN", "120"),
        ("THROTTLE_AUTH_PER_MIN", "5"),
        ("CORS_ORIGINS", "http://localhost:5173"),
        ("TRUSTED_PROXIES", ""),
    ];

    struct SavedEnv(Vec<(&'static str, Option<String>)>);

    impl SavedEnv {
        fn set(vars: &[(&'static str, &str)]) -> Self {
            let saved = SETTINGS_KEYS
                .iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect();
            for key in SETTINGS_KEYS {
                if !vars.iter().any(|(name, _)| name == key) {
                    std::env::remove_var(key);
                }
            }
            for (key, value) in vars {
                std::env::set_var(key, value);
            }
            Self(saved)
        }
    }

    impl Drop for SavedEnv {
        fn drop(&mut self) {
            for (key, old) in self.0.drain(..) {
                match old {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

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
        let _guard = ENV_LOCK.lock().unwrap();
        // Production with HASH_COST=1 must fail boot.
        {
            let vars = with_overrides(&[("NODE_ENV", "production"), ("HASH_COST", "1")]);
            let _env = SavedEnv::set(&vars);
            let err = Settings::load_from_env().expect_err("production HASH_COST=1 must fail boot");
            assert!(
                err.to_string().contains("HASH_COST"),
                "unexpected error: {err}"
            );
        }
        // Tests pin t=1 and must load outside production.
        {
            let vars = with_overrides(&[("NODE_ENV", "test"), ("HASH_COST", "1")]);
            let _env = SavedEnv::set(&vars);
            let settings = Settings::load_from_env().expect("test HASH_COST=1 must load");
            assert_eq!(settings.hash_cost, 1);
        }
    }

    #[test]
    fn missing_session_secret_fails_boot() {
        let _guard = ENV_LOCK.lock().unwrap();
        let mut vars = base_vars();
        vars.retain(|(name, _)| *name != "SESSION_SECRET");
        let _env = SavedEnv::set(&vars);
        let err = Settings::load_from_env().expect_err("missing SESSION_SECRET must fail boot");
        assert!(
            err.to_string().contains("session_secret"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn short_session_secret_fails_boot() {
        let _guard = ENV_LOCK.lock().unwrap();
        for secret in ["short", "0123456789abcdef0123456789abcde"] {
            assert!(secret.len() < MIN_SESSION_SECRET_BYTES);
            let vars = with_overrides(&[("SESSION_SECRET", secret)]);
            let _env = SavedEnv::set(&vars);
            let err = Settings::load_from_env().expect_err("short SESSION_SECRET must fail boot");
            assert!(
                err.to_string().contains("SESSION_SECRET"),
                "unexpected error: {err}"
            );
        }
        // Exactly 32 bytes loads.
        let vars = with_overrides(&[("SESSION_SECRET", "0123456789abcdef0123456789abcdef")]);
        let _env = SavedEnv::set(&vars);
        Settings::load_from_env().expect("32-byte SESSION_SECRET must load");
    }

    #[test]
    fn node_env_whitelist_and_port_rejects() {
        let _guard = ENV_LOCK.lock().unwrap();
        for bad_env in ["prod", "Production", "staging", ""] {
            let vars = with_overrides(&[("NODE_ENV", bad_env)]);
            let _env = SavedEnv::set(&vars);
            let err = Settings::load_from_env().expect_err("bad NODE_ENV must fail boot");
            assert!(
                err.to_string().contains("NODE_ENV"),
                "unexpected error for {bad_env:?}: {err}"
            );
        }
        for good_env in ["development", "test"] {
            let vars = with_overrides(&[("NODE_ENV", good_env)]);
            let _env = SavedEnv::set(&vars);
            Settings::load_from_env().expect("whitelisted NODE_ENV must load");
        }
        // Production needs HASH_COST>=2 to get past the cost floor.
        {
            let vars = with_overrides(&[("NODE_ENV", "production"), ("HASH_COST", "2")]);
            let _env = SavedEnv::set(&vars);
            Settings::load_from_env().expect("production NODE_ENV must load");
        }
        let vars = with_overrides(&[("PORT", "0")]);
        let _env = SavedEnv::set(&vars);
        let err = Settings::load_from_env().expect_err("PORT=0 must fail boot");
        assert!(err.to_string().contains("PORT"), "unexpected error: {err}");
    }

    /// AC: a database name the session store would reject (leading digit,
    /// hyphen, dot, space) is a clean config error — never a boot panic from
    /// `MySqlStore::with_schema_name`.
    #[test]
    fn rejects_database_names_the_session_store_would_refuse() {
        let _guard = ENV_LOCK.lock().unwrap();
        for (name, why) in [
            ("123kanpad", "leading digit"),
            ("kan-pad", "hyphen"),
            ("kan.pad", "dot"),
            ("kan pad", "space"),
        ] {
            let _env = SavedEnv::set(&with_db_name(name));

            let err = Settings::load_from_env().expect_err("invalid DB_NAME must fail boot");
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
        let _guard = ENV_LOCK.lock().unwrap();
        for name in ["kanpad", "_sqlx_test_a1b2c3", "kanpad_prod_1"] {
            let _env = SavedEnv::set(&with_db_name(name));
            Settings::load_from_env()
                .unwrap_or_else(|err| panic!("{name:?} must be accepted: {err}"));
        }
    }

    #[test]
    fn database_url_charset_and_derivation() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Explicit URL without the charset flag is rejected.
        {
            let vars = with_overrides(&[("DATABASE_URL", "mysql://u:p@h:3306/db")]);
            let _env = SavedEnv::set(&vars);
            let err =
                Settings::load_from_env().expect_err("charset-less DATABASE_URL must fail boot");
            assert!(
                err.to_string().contains("charset=utf8mb4"),
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
            let _env = SavedEnv::set(&vars);
            let settings = Settings::load_from_env().expect("parts-derived config must load");
            assert_eq!(
                settings.database_url(),
                "mysql://app:s3cret@db.internal:3307/kanpad?charset=utf8mb4"
            );
        }
        // Explicit DATABASE_URL wins over parts.
        {
            let vars = with_overrides(&[
                ("DATABASE_URL", "mysql://x:y@h:1/z?charset=utf8mb4"),
                ("DB_HOST", "ignored"),
            ]);
            let _env = SavedEnv::set(&vars);
            let settings = Settings::load_from_env().expect("explicit DATABASE_URL must load");
            assert_eq!(settings.database_url(), "mysql://x:y@h:1/z?charset=utf8mb4");
        }
    }

    #[test]
    fn cors_any_with_credentials_fails_and_prod_empty_fails_closed() {
        let _guard = ENV_LOCK.lock().unwrap();
        for origins in ["*", "http://localhost:5173, *"] {
            let vars = with_overrides(&[("CORS_ORIGINS", origins)]);
            let _env = SavedEnv::set(&vars);
            let err = Settings::load_from_env().expect_err("wildcard CORS must fail boot");
            assert!(
                err.to_string().contains("CORS_ORIGINS"),
                "unexpected error: {err}"
            );
        }
        // Prod + empty allowlist fails closed (HASH_COST=2 to isolate the CORS gate).
        {
            let vars = with_overrides(&[
                ("NODE_ENV", "production"),
                ("HASH_COST", "2"),
                ("CORS_ORIGINS", "   "),
            ]);
            let _env = SavedEnv::set(&vars);
            let err = Settings::load_from_env().expect_err("prod empty CORS must fail boot");
            assert!(
                err.to_string().contains("CORS_ORIGINS"),
                "unexpected error: {err}"
            );
        }
        // Non-prod empty allowlist loads (deny-all at runtime).
        {
            let vars = with_overrides(&[("CORS_ORIGINS", "")]);
            let _env = SavedEnv::set(&vars);
            let settings = Settings::load_from_env().expect("dev empty CORS must load");
            assert!(settings.cors_origin_list().is_empty());
        }
        // Malformed origins fail boot (fail fast, not silent deny).
        for origins in ["not-a-url", "localhost:5173", "ftp://files.example.com"] {
            let vars = with_overrides(&[("CORS_ORIGINS", origins)]);
            let _env = SavedEnv::set(&vars);
            let err = Settings::load_from_env().expect_err("malformed CORS origin must fail boot");
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
            let _env = SavedEnv::set(&vars);
            let settings = Settings::load_from_env().expect("multi-origin CORS must load");
            assert_eq!(
                settings.cors_origin_list(),
                vec![
                    "https://app.example.com".to_string(),
                    "https://admin.example.com".to_string()
                ]
            );
        }
    }

    #[test]
    fn trusted_proxies_empty_by_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        let vars = base_vars();
        let _env = SavedEnv::set(&vars);
        let settings = Settings::load_from_env().expect("defaults must load");
        assert!(settings.trusted_proxy_list().is_empty());
        assert!(settings.registration_enabled);
        assert_eq!(settings.throttle_global_per_min, 120);
        assert_eq!(settings.throttle_auth_per_min, 5);
    }

    #[test]
    fn debug_redacts_secrets() {
        let settings = Settings {
            session_secret: SecretString::new("super-secret-value-that-is-long".into()),
            database_url: Some("mysql://u:exposed-pw@h:1/d?charset=utf8mb4".to_string()),
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
        assert_eq!(settings.node_env, "test");
    }

    #[test]
    fn session_cookie_name_follows_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        for (env, name) in [
            ("development", "connect.sid"),
            ("test", "connect.sid"),
            ("production", "__Host-connect.sid"),
        ] {
            // Production refuses HASH_COST<2 at boot, so pin t=2 there.
            let vars = if env == "production" {
                with_overrides(&[("NODE_ENV", env), ("HASH_COST", "2")])
            } else {
                with_overrides(&[("NODE_ENV", env)])
            };
            let _env = SavedEnv::set(&vars);
            let settings = Settings::load_from_env().expect("env must load");
            assert_eq!(settings.session_cookie_name(), name, "env: {env}");
            assert_eq!(settings.is_production(), env == "production");
        }
        // Struct-level (no env): same rule.
        let mut settings = Settings::default();
        assert_eq!(settings.session_cookie_name(), "connect.sid");
        settings.node_env = "production".to_string();
        assert_eq!(settings.session_cookie_name(), "__Host-connect.sid");
    }
}
