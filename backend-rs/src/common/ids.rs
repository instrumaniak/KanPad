//! Newtype IDs (`UserId`, `BoardId`, …).
//!
//! Every table PK / FK is an `INT`, so IDs are `i32` newtypes: transparent
//! over the wire (JSON number), transparent in sqlx (bound directly, never
//! `.0`-at-every-site), and `i32`-shaped in OpenAPI. Handlers take
//! `Path<BoardId>` etc. so malformed ids fail int-parsing with 400.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Defines one transparent `i32` id newtype with serde + sqlx + OpenAPI impls.
macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, ToSchema,
        )]
        #[serde(transparent)]
        #[sqlx(transparent)]
        #[schema(value_type = i32)]
        pub struct $name(pub i32);

        impl $name {
            /// Unwrap to the raw key (for logging and tests).
            pub fn get(self) -> i32 {
                self.0
            }
        }

        impl From<i32> for $name {
            fn from(value: i32) -> Self {
                Self(value)
            }
        }

        impl From<$name> for i32 {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }
    };
}

define_id!(UserId, "Primary key of `users`.");
define_id!(ProjectId, "Primary key of `projects`.");
define_id!(BoardId, "Primary key of `boards`.");
define_id!(ColumnId, "Primary key of `columns`.");
define_id!(CardId, "Primary key of `cards`.");
define_id!(LabelId, "Primary key of `labels`.");
define_id!(ChecklistId, "Primary key of `checklists`.");
define_id!(ChecklistItemId, "Primary key of `checklist_items`.");
define_id!(NoteId, "Primary key of `notes`.");
define_id!(TagId, "Primary key of `tags`.");

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time proof that every newtype binds/decodes as MySQL `INT`
    /// (no `.0`-at-every-site fallback to raw `i32` in store code).
    fn assert_mysql_int<T>()
    where
        T: sqlx::Type<sqlx::MySql>
            + for<'q> sqlx::Encode<'q, sqlx::MySql>
            + for<'r> sqlx::Decode<'r, sqlx::MySql>,
    {
    }

    #[test]
    fn newtypes_encode_as_mysql_int() {
        assert_mysql_int::<UserId>();
        assert_mysql_int::<ProjectId>();
        assert_mysql_int::<BoardId>();
        assert_mysql_int::<ColumnId>();
        assert_mysql_int::<CardId>();
        assert_mysql_int::<LabelId>();
        assert_mysql_int::<ChecklistId>();
        assert_mysql_int::<ChecklistItemId>();
        assert_mysql_int::<NoteId>();
        assert_mysql_int::<TagId>();
    }

    #[test]
    fn newtypes_serialize_transparently() {
        assert_eq!(serde_json::to_string(&BoardId(7)).unwrap(), "7");
        assert_eq!(serde_json::from_str::<CardId>("42").unwrap(), CardId(42));
        assert_eq!(UserId::from(3).get(), 3);
        assert_eq!(i32::from(TagId(9)), 9);
    }
}
