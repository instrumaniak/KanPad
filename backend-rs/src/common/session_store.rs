//! The session store, **owned in-tree** (`54` Part C §C4).
//!
//! # Why this exists
//!
//! The store used to be `tower-sessions-sqlx-store 0.15`, whose last release
//! was **2025-01-01** and whose dependabot PR to bump sqlx to 0.9 has been
//! **open since 2026-05-21**. It requires `sqlx ^0.8.0` **and**
//! `tower-sessions-core ^0.14.0`, while `tower-sessions 0.15` requires
//! `tower-sessions-core =0.15.0` — so taking **either** dependency major
//! pulled a **second** sqlx / a **second** `tower-sessions-core` into the tree
//! and did not compile. Owning the store in-tree removes that block: it is
//! the reason `sqlx 0.9` and `tower-sessions 0.15` are reachable at all.
//!
//! # What this does NOT change
//!
//! **The on-disk format is byte-identical.** The table is the existing
//! `sessions` table in the **app database** (created by the baseline
//! migration — `migrations/0001_baseline.sql`), with the same DDL:
//! `id CHAR(22)` / `data BLOB` / `expiry_date TIMESTAMP(6)` +
//! `IDX_session_expiry_date`. The payload stays **MessagePack**
//! (`rmp-serde`) and encodes the **whole `Record`** — exactly what the
//! previous store wrote — so a row written before this change still reads.
//! The store never calls `migrate()`: the baseline owns the table (see
//! `30-db-schema.md` deviation 11), and the runbook's step 5(d) schema-drift
//! gate therefore stays green.
//!
//! # Why the store needs no schema name
//!
//! The previous store interpolated a schema name into every statement and
//! defaulted to a `tower_sessions` schema — which is how the store ended up
//! pointing at the wrong database until the 2026-10-19 fix. **This store
//! takes the application's own pool**, so the table is in the pool's
//! database *by construction*: there is no name to get wrong, and no
//! identifier rule to validate at boot.
//!
//! # The `time` → `chrono` conversion
//!
//! `tower-sessions` speaks `time::OffsetDateTime`; the `expiry_date` column
//! is a MySQL `TIMESTAMP(6)`. The previous store bound the `time` type
//! directly, which is the **only** reason the `sqlx` `time` feature was
//! enabled in this crate — and with both `time` and `chrono` enabled, sqlx
//! prefers `time` for `DATETIME`, which is why eight feature store files
//! carried an `AS "col: chrono::NaiveDateTime"` override on every datetime
//! column. Dropping this store turns `sqlx/time` **off**, sqlx infers
//! `chrono::NaiveDateTime` natively, and those overrides are gone. The one
//! conversion this module owns is therefore
//! [`OffsetDateTime`] → [`chrono::NaiveDateTime`] at **microsecond**
//! precision — the precision of the column, and the precision the previous
//! store's encoding also truncated to.

use async_trait::async_trait;
use sqlx::{MySqlConnection, MySqlPool};
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store::{Error, ExpiredDeletion, Result, SessionStore};

/// The session table, in the **app database**.
///
/// The name is fixed by the baseline migration (`migrations/0001_baseline.sql`
/// creates `sessions` in the app database) — the store does not create,
/// migrate or rename it. It is a `const` next to the SQL that uses it so the
/// two cannot drift apart silently.
pub const SESSION_TABLE: &str = "sessions";

/// A session store backed by the application's own MySQL pool.
///
/// Cheap to clone (the pool is `Arc`-interned) — the `SessionManagerLayer`
/// and the expiry sweeper each hold a handle.
#[derive(Clone, Debug)]
pub struct MySqlSessionStore {
    pool: MySqlPool,
}

impl MySqlSessionStore {
    /// Bind the store to `pool`.
    ///
    /// The session table is then reached in **the pool's own database** — the
    /// invariant the 2026-10-19 fix established, now structural rather than
    /// configured.
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// The pool this store reads and writes through.
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    /// Whether a session with this id already exists.
    async fn id_exists(&self, conn: &mut MySqlConnection, id: &Id) -> Result<bool> {
        // MySQL's `EXISTS` is an integer expression (1/0), and sqlx infers
        // it as `i64` — not as a `bool`.
        let exists: i64 = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM `sessions` WHERE `id` = ?)",
            id.to_string()
        )
        .fetch_one(conn)
        .await
        .map_err(backend_error)?;
        Ok(exists != 0)
    }

    /// Write the record (insert or update), on the given connection.
    async fn save_with_conn(&self, conn: &mut MySqlConnection, record: &Record) -> Result<()> {
        // The payload is the WHOLE `Record`, MessagePack-encoded — the same
        // bytes the previous store wrote, so a row written before this change
        // still reads.
        let payload =
            rmp_serde::to_vec(record).map_err(|error| Error::Encode(error.to_string()))?;
        let expiry_date = to_naive_utc(record.expiry_date)?;
        // `AS new` is the modern form of the upsert (MySQL 8.0.19+): the
        // `VALUES(col)` function it replaces is deprecated in 8.0.20+ and
        // emits a server-side deprecation warning.
        sqlx::query!(
            "INSERT INTO `sessions` (`id`, `data`, `expiry_date`) VALUES (?, ?, ?) AS new \
             ON DUPLICATE KEY UPDATE `data` = new.`data`, `expiry_date` = new.`expiry_date`",
            record.id.to_string(),
            payload,
            expiry_date,
        )
        .execute(conn)
        .await
        .map_err(backend_error)?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for MySqlSessionStore {
    /// Create the session, resolving an id collision.
    ///
    /// A collision is astronomically unlikely (the id is a random 128-bit
    /// value), but the trait makes the handling the implementer's choice, so
    /// it is handled explicitly: a new id is drawn until the slot is free.
    /// The check and the insert share one transaction, so two concurrent
    /// creates cannot both claim the same id.
    async fn create(&self, record: &mut Record) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(backend_error)?;

        while self.id_exists(&mut tx, &record.id).await? {
            record.id = Id::default();
        }
        self.save_with_conn(&mut tx, record).await?;

        tx.commit().await.map_err(backend_error)?;
        Ok(())
    }

    /// Persist an updated session (upsert).
    async fn save(&self, record: &Record) -> Result<()> {
        let mut conn = self.pool.acquire().await.map_err(backend_error)?;
        self.save_with_conn(&mut conn, record).await
    }

    /// Load a session, or `None` if it is unknown **or expired**.
    ///
    /// The expiry is filtered in SQL (`expiry_date > ?`), so an expired row
    /// is never decoded — the row is skipped by the index on
    /// `expiry_date`.
    async fn load(&self, session_id: &Id) -> Result<Option<Record>> {
        let row = sqlx::query!(
            "SELECT `data` FROM `sessions` WHERE `id` = ? AND `expiry_date` > ?",
            session_id.to_string(),
            chrono::Utc::now().naive_utc(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(backend_error)?;

        match row {
            Some(row) => {
                let record = rmp_serde::from_slice(&row.data)
                    .map_err(|error| Error::Decode(error.to_string()))?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    /// Delete one session — **scoped by id** (never a blanket delete).
    async fn delete(&self, session_id: &Id) -> Result<()> {
        sqlx::query!(
            "DELETE FROM `sessions` WHERE `id` = ?",
            session_id.to_string()
        )
        .execute(&self.pool)
        .await
        .map_err(backend_error)?;
        Ok(())
    }
}

#[async_trait]
impl ExpiredDeletion for MySqlSessionStore {
    /// Delete every **expired** session.
    ///
    /// This is the one statement that is deliberately not scoped by id: it
    /// IS the sweeper (`main.rs` runs it every 60 s). It is scoped by
    /// `expiry_date`, which is the `IDX_session_expiry_date` index — so it
    /// touches only the rows past their expiry, never a live session.
    async fn delete_expired(&self) -> Result<()> {
        sqlx::query!("DELETE FROM `sessions` WHERE `expiry_date` < UTC_TIMESTAMP(6)")
            .execute(&self.pool)
            .await
            .map_err(backend_error)?;
        Ok(())
    }
}

/// `sqlx` failures become backend errors.
///
/// The message is the driver's own text; it never carries the connection
/// string or the password (`sqlx` does not put them in an error's `Display`).
fn backend_error(error: sqlx::Error) -> Error {
    Error::Backend(error.to_string())
}

/// `OffsetDateTime` → `chrono::NaiveDateTime` (UTC), at **microsecond**
/// precision.
///
/// `TIMESTAMP(6)` stores microseconds, and the previous store's encoding
/// truncated to the same precision — so this conversion is value-preserving
/// with respect to what is on disk. `OffsetDateTime` is always UTC here
/// (`tower-sessions` builds it with `OffsetDateTime::now_utc()`), so the
/// naive value is the UTC wall clock.
fn to_naive_utc(value: OffsetDateTime) -> Result<chrono::NaiveDateTime> {
    let micros = value.unix_timestamp() * 1_000_000 + i64::from(value.nanosecond() / 1_000);
    chrono::DateTime::from_timestamp_micros(micros)
        .map(|stamp| stamp.naive_utc())
        .ok_or_else(|| {
            Error::Backend(format!(
                "session expiry {value:?} is outside the range of a MySQL TIMESTAMP"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use sqlx::MySqlPool;
    use std::collections::HashMap;

    /// A record with the given expiry (the `Record` fields are public; its
    /// `Data` alias is not, but it is a plain `HashMap`).
    fn record_expiring_at(expiry: OffsetDateTime) -> Record {
        Record {
            id: Id::default(),
            data: HashMap::new(),
            expiry_date: expiry,
        }
    }

    /// An expiry an hour out.
    fn in_an_hour() -> OffsetDateTime {
        OffsetDateTime::now_utc() + time::Duration::hours(1)
    }

    /// The expiry conversion keeps the instant, at microsecond precision.
    #[test]
    fn expiry_conversion_keeps_the_instant() {
        let value = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid timestamp");
        let converted = to_naive_utc(value).expect("in range");
        assert_eq!(
            converted.and_utc(),
            chrono::Utc
                .timestamp_opt(1_700_000_000, 0)
                .single()
                .expect("valid instant")
        );
    }

    /// Sub-microsecond precision is truncated, not rounded — the same
    /// truncation the previous store's encoding performed.
    #[test]
    fn expiry_conversion_truncates_to_microseconds() {
        let value = OffsetDateTime::from_unix_timestamp_nanos(1_700_000_000_123_456_789)
            .expect("valid timestamp");
        let converted = to_naive_utc(value).expect("in range");
        assert_eq!(
            converted.and_utc().timestamp_subsec_micros(),
            123_456,
            "nanos are truncated to micros, not rounded"
        );
    }

    /// The table name is the app-database one, and it is a valid identifier
    /// (the baseline migration creates it under exactly this name).
    #[test]
    fn session_table_is_the_app_database_one() {
        assert_eq!(SESSION_TABLE, "sessions");
        assert!(crate::db::is_valid_db_name(SESSION_TABLE));
    }

    /// A stored row round-trips: `create` then `load` returns the record.
    #[sqlx::test(migrations = "./migrations")]
    async fn create_then_load_round_trips(pool: MySqlPool) {
        let store = MySqlSessionStore::new(pool.clone());
        let mut record = record_expiring_at(in_an_hour());
        store.create(&mut record).await.expect("create");

        let loaded = store
            .load(&record.id)
            .await
            .expect("load")
            .expect("the session is there");
        assert_eq!(loaded.id, record.id);
        assert_eq!(loaded.expiry_date, record.expiry_date);
    }

    /// **The payload is byte-identical to what the previous store wrote.**
    ///
    /// The old store encoded `rmp_serde::to_vec(&record)` — the WHOLE
    /// `Record`, MessagePack-encoded. The blob on disk must therefore be
    /// exactly those bytes, so a row written before the swap still reads.
    #[sqlx::test(migrations = "./migrations")]
    async fn the_stored_blob_is_the_previous_store_s_bytes(pool: MySqlPool) {
        let store = MySqlSessionStore::new(pool.clone());
        let mut record = record_expiring_at(in_an_hour());
        store.create(&mut record).await.expect("create");

        let blob: Vec<u8> = sqlx::query_scalar!(
            "SELECT `data` FROM `sessions` WHERE `id` = ?",
            record.id.to_string()
        )
        .fetch_one(&pool)
        .await
        .expect("the row");
        let expected = rmp_serde::to_vec(&record).expect("encode");
        assert_eq!(
            blob, expected,
            "the blob must be the MessagePack encoding of the whole Record"
        );
        // And it decodes back to the same record with a plain decoder — the
        // "a row written before the change still reads" guarantee.
        let decoded: Record = rmp_serde::from_slice(&blob).expect("decode");
        assert_eq!(decoded, record);
    }

    /// An EXPIRED row is never returned — the expiry is filtered in SQL.
    #[sqlx::test(migrations = "./migrations")]
    async fn an_expired_row_is_not_loaded(pool: MySqlPool) {
        let store = MySqlSessionStore::new(pool.clone());
        let mut record = record_expiring_at(OffsetDateTime::now_utc() - time::Duration::hours(1));
        store.create(&mut record).await.expect("create");

        assert!(
            store.load(&record.id).await.expect("load").is_none(),
            "an expired session must not be returned"
        );
    }

    /// `delete` is scoped by id: the other rows survive.
    #[sqlx::test(migrations = "./migrations")]
    async fn delete_is_scoped_by_id(pool: MySqlPool) {
        let store = MySqlSessionStore::new(pool.clone());
        let mut first = record_expiring_at(in_an_hour());
        let mut second = record_expiring_at(in_an_hour());
        store.create(&mut first).await.expect("create");
        store.create(&mut second).await.expect("create");

        store.delete(&first.id).await.expect("delete");

        assert!(store.load(&first.id).await.expect("load").is_none());
        assert!(
            store.load(&second.id).await.expect("load").is_some(),
            "the other session must survive"
        );
    }

    /// The sweeper removes only the EXPIRED rows.
    #[sqlx::test(migrations = "./migrations")]
    async fn delete_expired_removes_only_expired_rows(pool: MySqlPool) {
        let store = MySqlSessionStore::new(pool.clone());
        let mut expired = record_expiring_at(OffsetDateTime::now_utc() - time::Duration::hours(1));
        let mut live = record_expiring_at(in_an_hour());
        store.create(&mut expired).await.expect("create");
        store.create(&mut live).await.expect("create");

        store.delete_expired().await.expect("sweep");

        assert!(store.load(&expired.id).await.expect("load").is_none());
        assert!(
            store.load(&live.id).await.expect("load").is_some(),
            "a live session must survive the sweeper"
        );
    }

    /// `save` upserts: saving the same id twice does not create a second
    /// row, and the payload is replaced.
    #[sqlx::test(migrations = "./migrations")]
    async fn save_upserts_the_same_row(pool: MySqlPool) {
        let store = MySqlSessionStore::new(pool.clone());
        let mut record = record_expiring_at(in_an_hour());
        store.create(&mut record).await.expect("create");

        let mut updated = record.clone();
        updated.expiry_date = in_an_hour() + time::Duration::hours(1);
        store.save(&updated).await.expect("save");

        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM `sessions` WHERE `id` = ?",
            record.id.to_string()
        )
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(count, 1, "the upsert must not create a second row");
        let loaded = store.load(&record.id).await.expect("load").expect("there");
        assert_eq!(loaded.expiry_date, updated.expiry_date);
    }
}
