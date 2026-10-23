//! Pool defaults + MySQL identifier rules (T03 skeleton). No `query!` here,
//! so no `.sqlx/` entries.

use std::time::Duration;

use sqlx::mysql::MySqlPoolOptions;

/// Canonical pool defaults per 10-architecture.md DB pool.
pub fn pool_options() -> MySqlPoolOptions {
    MySqlPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(600)))
        .max_lifetime(Some(Duration::from_secs(30 * 60)))
        .test_before_acquire(true)
        // A slow acquire must also be a tracing event; `log::LevelFilter` is
        // the type sqlx expects.
        .acquire_time_level(log::LevelFilter::Info)
}

/// Whether `name` is usable as a plain database identifier.
///
/// The rule is STRICTER than MySQL itself: a database name starting with a
/// digit, or containing `-`, `.`, or a space, is legal SQL but rejected here.
///
/// **Why the rule still exists (it used to be the store's rule).** It was
/// inherited from `tower-sessions-sqlx-store`'s `with_schema_name`, which
/// interpolated the name into every statement and validated it. The
/// in-tree store (T65) takes the pool and interpolates **nothing**, so it no
/// longer needs the name to be identifier-safe — but the screen stays, for
/// two reasons that outlive the store: (a) the database name reaches
/// operator tooling that does not quote identifiers (`mysql -D`, dumps, the
/// runbook's `CREATE DATABASE`), where a name needing quoting is a trap;
/// (b) `Settings::validate` is the place where a bad name becomes a clean
/// boot error instead of a surprise at the first query. Mirrored here so
/// that screen can be expressed in one place.
pub fn is_valid_db_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|first| first.is_alphabetic() || first == '_')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

#[cfg(test)]
mod tests {
    use super::is_valid_db_name;

    #[test]
    fn accepts_identifiers_the_store_accepts() {
        for name in [
            "kanpad",         // the default DB_NAME
            "_sqlx_test_abc", // #[sqlx::test] per-test databases
            "_leading",
            "with_underscores_123",
            "dollar$sign",
            "kanpad_prod_1",
        ] {
            assert!(is_valid_db_name(name), "{name:?} must be accepted");
        }
    }

    #[test]
    fn rejects_names_mysql_allows_but_the_store_does_not() {
        // These are legal MySQL schema names but unusable as plain
        // identifiers, so `Settings::validate` must reject them — otherwise
        // the first operator tool that does not quote identifiers breaks on
        // a name that the boot screen let through.
        for name in [
            "123kanpad", // leading digit
            "kan-pad",   // hyphen needs quoting
            "kan.pad",   // dot needs quoting
            "kan pad",   // space needs quoting
            "",          // empty
            "kan@pad",
        ] {
            assert!(!is_valid_db_name(name), "{name:?} must be rejected");
        }
    }

    #[test]
    fn non_ascii_letters_count_as_alphabetic_like_the_store() {
        // The crate checks `char::is_alphabetic`, which is Unicode-aware;
        // mirror it exactly rather than restricting to ASCII.
        assert!(is_valid_db_name("kanpäd"));
    }
}
