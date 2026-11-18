//! Shared retry matcher for transient lock/duplicate-key failures (T010, D3).
//!
//! Moved out of `cards::routes` when the same match the card move needs also
//! covers duplicate-`sort_key` inserts. Matched by ERROR NUMBER — never by
//! SQLSTATE class, because `1213` (deadlock) is `40001` but `1205`
//! (lock-wait timeout) is `HY000`, so a `40001`-only matcher silently misses
//! every lock-wait timeout. (The one SQLSTATE in the tree is a documented
//! test-stub fallback in `is_lock_class_sqlx_error`, not a matcher rule.)
//! A duplicate key is `1062` (SQLSTATE `23000`) — but only when the
//! violated constraint is one of this schema's `UQ_*_sort_key` unique
//! indexes; unrelated unique violations (`UQ_labels_user_name`,
//! `card_labels`, …) are permanent and must never be retried.
//!
//! The `1062` scoping reads the `for key '…'` suffix of the MySQL error
//! message (MariaDB may prefix it with `db.table.`) and checks that the
//! final dot-component starts with `UQ_` and ends with `_sort_key` — see
//! [`is_duplicate_sort_key_message`]. If scoping ever proves unworkable, the
//! documented fallback is a workspace-wide `1062` retry — unrelated
//! duplicates would then burn the retry budget before surfacing a 409
//! (recorded in `specs/tasks/T010-*.md` §Notes).

use crate::error::AppError;

/// Maximum attempts for a retry-wrapped transaction (first try plus four
/// transient-failure retries).
pub const MAX_ATTEMPTS: u32 = 5;

/// Whether a MySQL ERROR NUMBER is lock-class contention: `1213` (deadlock)
/// or `1205` (lock-wait timeout). Single source of truth shared by the retry
/// matcher and the HTTP boundary (`error.rs` renders these as 503 +
/// `Retry-After: 2` — the class means "lost a race, retry"). Scoped `1062` is
/// NOT lock-class — it stays 409 (class 23, the unique index doing its job).
pub fn is_lock_class_number(number: u16) -> bool {
    matches!(number, 1213 | 1205)
}

/// The `sqlx::Error`-typed half of the lock-class predicate, exported for the
/// `Db` arm of the HTTP boundary (which matches the inner error).
///
/// The driver's concrete error is downcast first (authoritative: `1205` is
/// `HY000`, so SQLSTATE alone cannot see it). The `40001` SQLSTATE fallback
/// catches the serialization-failure class when the concrete type is not
/// visible (e.g. a unit-test stub); `40001` is contention by definition and
/// never anything else.
pub fn is_lock_class_sqlx_error(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(database) = error else {
        return false;
    };
    if let Some(mysql) = database
        .as_error()
        .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
    {
        return is_lock_class_number(mysql.number());
    }
    database.code().as_deref() == Some("40001")
}

/// Whether an error is a lock-class failure (see [`is_lock_class_sqlx_error`]).
pub fn is_lock_class_error(error: &AppError) -> bool {
    let AppError::Db(inner) = error else {
        return false;
    };
    is_lock_class_sqlx_error(inner)
}

/// Whether an error means "the transaction lost a race and may succeed on
/// retry": MySQL `1213` (deadlock) or `1205` (lock-wait timeout), or a
/// duplicate key `1062` on a `UQ_*_sort_key` constraint.
pub fn is_transient_lock_error(error: &AppError) -> bool {
    is_lock_class_error(error)
        || matches!(
            error,
            AppError::Db(sqlx::Error::Database(database))
                if database
                    .as_error()
                    .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                    .is_some_and(|mysql| {
                        mysql.number() == 1062 && is_duplicate_sort_key_message(mysql.message())
                    })
        )
}

/// `1062` scoped to the `UQ_*_sort_key` constraint names: the message
/// carries `for key '<constraint>'` (MariaDB may prefix `db.table.` — the
/// last dot-component is matched).
fn is_duplicate_sort_key_message(message: &str) -> bool {
    let Some(start) = message.find("for key '") else {
        return false;
    };
    let key_start = start + "for key '".len();
    let Some(end_rel) = message[key_start..].find('\'') else {
        return false;
    };
    let key_name = &message[key_start..key_start + end_rel];
    let suffix = key_name.rsplit('.').next().unwrap_or(key_name);
    suffix.starts_with("UQ_") && suffix.ends_with("_sort_key")
}

#[cfg(test)]
mod tests {
    use super::{is_duplicate_sort_key_message, is_lock_class_number, is_transient_lock_error};

    #[test]
    fn lock_class_numbers_are_recognised() {
        for number in [1213, 1205] {
            assert!(is_lock_class_number(number), "{number} must be lock-class");
        }
        for number in [1062, 2003, 1210, 0, 9999] {
            assert!(
                !is_lock_class_number(number),
                "{number} must not be lock-class"
            );
        }
    }

    #[test]
    fn unrelated_1062_stays_permanent_in_the_matcher() {
        // Regression pin for the fallback arm: a stub that cannot downcast to
        // the driver's concrete type is not transient unless its SQLSTATE is
        // exactly `40001`.
        let stub_1062 =
            crate::error::AppError::Db(sqlx::Error::Database(Box::new(StubForMatcher {
                code: "23000",
                message: "Duplicate entry 'x' for key 'uq'",
            })));
        assert!(!is_transient_lock_error(&stub_1062));
    }

    #[test]
    fn stub_40001_is_lock_class_via_the_sqlstate_fallback() {
        let stub_40001 =
            crate::error::AppError::Db(sqlx::Error::Database(Box::new(StubForMatcher {
                code: "40001",
                message: "Deadlock found",
            })));
        assert!(is_transient_lock_error(&stub_40001));
    }

    #[derive(Debug)]
    struct StubForMatcher {
        code: &'static str,
        message: &'static str,
    }

    impl std::fmt::Display for StubForMatcher {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "stub: {}", self.message)
        }
    }

    impl std::error::Error for StubForMatcher {}

    impl sqlx::error::DatabaseError for StubForMatcher {
        fn message(&self) -> &str {
            self.message
        }
        fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
            Some(self.code.into())
        }
        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::Other
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
    }

    #[test]
    fn uq_sort_key_constraints_are_recognised() {
        for constraint in [
            "UQ_columns_board_sort_key",
            "UQ_cards_column_sort_key",
            "UQ_checklist_items_checklist_sort_key",
        ] {
            let message = format!("Duplicate entry '5-a0' for key '{constraint}'");
            assert!(
                is_duplicate_sort_key_message(&message),
                "constraint {constraint} must be retried"
            );
            // MariaDB may prefix with the db/table path.
            let message = format!("Duplicate entry '5-a0' for key 'kanpad.cards.{constraint}'");
            assert!(
                is_duplicate_sort_key_message(&message),
                "prefixed constraint {constraint} must be retried"
            );
        }
    }

    #[test]
    fn unrelated_constraints_are_not_retried() {
        for message in [
            "Duplicate entry 'work' for key 'UQ_labels_user_name'",
            "Duplicate entry '1-2' for key 'card_labels'",
            "Duplicate entry 'x' for key 'UQ_tags_user_name'",
            "Duplicate entry 'x' for key 'IDX_cards_column_position'",
            "Duplicate entry 'x' for key 'UQ_columns_board_sort_key_extra'", // not the exact suffix shape
            "some other database error entirely",
        ] {
            assert!(
                !is_duplicate_sort_key_message(message),
                "unrelated message must not be retried: {message}"
            );
        }
    }
}
