//! Shared retry matcher for transient lock/duplicate-key failures (T010, D3).
//!
//! Moved out of `cards::routes` when the same match the card move needs also
//! covers duplicate-`sort_key` inserts. Matched by ERROR NUMBER, never
//! SQLSTATE: `1213` (deadlock) is `40001` but `1205` (lock-wait timeout) is
//! `HY000`, so a `40001`-only matcher silently misses every lock-wait
//! timeout. A duplicate key is `1062` (SQLSTATE `23000`) — but only when the
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

/// Whether an error means "the transaction lost a race and may succeed on
/// retry": MySQL `1213` (deadlock) or `1205` (lock-wait timeout), or a
/// duplicate key `1062` on a `UQ_*_sort_key` constraint.
pub fn is_transient_lock_error(error: &AppError) -> bool {
    let AppError::Db(sqlx::Error::Database(database)) = error else {
        return false;
    };
    database
        .as_error()
        .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
        .is_some_and(|mysql| {
            matches!(mysql.number(), 1213 | 1205)
                || (mysql.number() == 1062 && is_duplicate_sort_key_message(mysql.message()))
        })
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
    use super::is_duplicate_sort_key_message;

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
