//! Pool defaults + MySQL identifier rules + canonical executor bound
//! (T03 skeleton). No `query!` here, so no `.sqlx/` entries.

use std::time::Duration;

use sqlx::MySqlExecutor;
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
}

/// Whether `name` is acceptable to `tower-sessions-sqlx-store`'s
/// `MySqlStore::with_schema_name`, which validates every name against this
/// exact rule (the crate's own `is_valid_identifier` is private).
///
/// It is STRICTER than MySQL itself: a schema name starting with a digit, or
/// containing `-`, `.`, or a space, is legal SQL but rejected here. The
/// session table lives in the app database (`30-db-schema.md` deviation 11),
/// so the database name must satisfy the store's rule or every session query
/// fails at runtime. Mirrored here so `Settings::validate`
/// can reject such a config cleanly at load instead of letting store
/// construction panic later.
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

/// Compile-time proof of the canonical executor bound: every feature store
/// fn is generic over `sqlx::MySqlExecutor<'e>` (one form everywhere), so the
/// same fn accepts `&MySqlPool` and `&mut Transaction<'_, MySql>`.
pub async fn with_executor<'e, E>(_ex: E) -> Result<(), sqlx::Error>
where
    E: MySqlExecutor<'e>,
{
    Ok(())
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
        // These are legal MySQL schema names; `MySqlStore::with_schema_name`
        // refuses them, so `Settings::validate` must too — otherwise the
        // session store panics at construction on boot.
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
