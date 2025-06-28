//! A `sqlx` implementation of the `sourcerer` store traits.
//!
//! This module provides `sqlx`-based implementations of the `EventStore` and
//! `SnapshotStore` traits, designed for PostgreSQL. Compile it with the
//! `postgres-storage` cargo feature.
#![allow(clippy::missing_errors_doc)]

use std::marker::PhantomData;

use crate::{
    Aggregate, Error, Event, EventStore, Result, StoredEvent,
    snapshot::{SnapshotStore, StoredSnapshot},
    upcaster,
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::PgPool;
use tracing::instrument;

/// Maps `sqlx::Error` into this crate's `Error`.
fn to_store_error(e: sqlx::Error) -> Error {
    Error::Store(e.to_string())
}

/// Maps `serde_json::Error` into this crate's `Error`.
fn to_serde_error(e: serde_json::Error) -> Error {
    Error::Store(e.to_string())
}

/// A `sqlx`-backed event store for PostgreSQL.
#[derive(Debug, Clone)]
pub struct SqlxEventStore<A: Aggregate> {
    pool: PgPool,
    _phantom: PhantomData<A>,
}

impl<A: Aggregate> SqlxEventStore<A> {
    /// Creates a new `SqlxEventStore`.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            _phantom: PhantomData,
        }
    }

    /// Ensures the `events` table exists.
    #[instrument(skip(self))]
    pub async fn setup(&self) -> sqlx::Result<()> {
        sqlx::query(
            r#"
                CREATE TABLE IF NOT EXISTS events (
                    aggregate_id TEXT NOT NULL,
                    version BIGINT NOT NULL,
                    event_version SMALLINT NOT NULL,
                    event_type TEXT NOT NULL,
                    payload JSONB NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    PRIMARY KEY (aggregate_id, version)
                );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl<A> EventStore<A> for SqlxEventStore<A>
where
    A: Aggregate,
    A::Event: Serialize + DeserializeOwned + Send + Sync,
    A::Id: Clone + Serialize + Send + Sync,
{
    #[instrument(skip(self, events), fields(id = ?id))]
    async fn append(
        &self,
        id: &A::Id,
        expected_version: i64,
        events: Vec<A::Event>,
    ) -> Result<Vec<StoredEvent<A::Event>>> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        let aggregate_id = id.to_string();
        let versions: Vec<i64> = (1..=events.len() as i64)
            .map(|i| expected_version + i)
            .collect();

        let payloads: Vec<serde_json::Value> = events
            .iter()
            .map(|e| serde_json::to_value(e).map_err(to_serde_error))
            .collect::<Result<_>>()?;
        let event_types: Vec<String> = events.iter().map(|e| e.event_type().to_owned()).collect();
        let event_versions: Vec<i16> = events.iter().map(|e| e.event_version() as i16).collect();

        let mut tx = self.pool.begin().await.map_err(to_store_error)?;

        // Optimistic concurrency check.
        let current_version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM events WHERE aggregate_id = $1")
                .bind(&aggregate_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(to_store_error)?;

        if current_version.unwrap_or(0) != expected_version {
            return Err(Error::Conflict);
        }

        // Bulk insert.
        sqlx::query(
            r#"
            INSERT INTO events (aggregate_id, version, payload, event_type, event_version)
            SELECT $1, v, p, t, ev
            FROM UNNEST($2::BIGINT[], $3::JSONB[], $4::TEXT[], $5::SMALLINT[]) AS x(v, p, t, ev)
            "#,
        )
        .bind(&aggregate_id)
        .bind(&versions)
        .bind(&payloads)
        .bind(&event_types)
        .bind(&event_versions)
        .execute(&mut *tx)
        .await
        .map_err(to_store_error)?;

        tx.commit().await.map_err(to_store_error)?;

        Ok(versions
            .into_iter()
            .zip(events.into_iter())
            .zip(event_types.into_iter())
            .map(|((version, event), event_type)| {
                StoredEvent::new(
                    aggregate_id.clone(),
                    version,
                    event.event_version(),
                    event_type,
                    event,
                )
            })
            .collect())
    }

    #[instrument(skip(self), fields(id = ?id))]
    async fn load(&self, id: &A::Id) -> Result<Vec<StoredEvent<A::Event>>> {
        let rows: Vec<(i64, i16, String, serde_json::Value)> = sqlx::query_as(
            "SELECT version, event_version, event_type, payload FROM events WHERE aggregate_id = $1 ORDER BY version",
        )
        .bind(id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(to_store_error)?;

        rows.into_iter()
            .map(|(version, ev_version, ev_type, payload)| {
                let event: A::Event = serde_json::from_value(payload).map_err(to_serde_error)?;
                Ok(StoredEvent::new(
                    id.to_string(),
                    version,
                    ev_version as u16,
                    ev_type,
                    event,
                ))
            })
            .collect()
    }

    #[instrument(skip(self), fields(id = ?id, version))]
    async fn load_from(&self, id: &A::Id, version: i64) -> Result<Vec<StoredEvent<A::Event>>> {
        let rows: Vec<(i64, i16, String, serde_json::Value)> = sqlx::query_as(
            "SELECT version, event_version, event_type, payload FROM events WHERE aggregate_id = $1 AND version > $2 ORDER BY version",
        )
        .bind(id.to_string())
        .bind(version)
        .fetch_all(&self.pool)
        .await
        .map_err(to_store_error)?;

        rows.into_iter()
            .map(|(version, ev_version, ev_type, payload)| {
                let event: A::Event = serde_json::from_value(payload).map_err(to_serde_error)?;
                Ok(StoredEvent::new(
                    id.to_string(),
                    version,
                    ev_version as u16,
                    ev_type,
                    event,
                ))
            })
            .collect()
    }

    #[instrument(skip(self), fields(id = ?id, version))]
    async fn load_raw(&self, id: &A::Id, version: i64) -> Result<Vec<upcaster::RawStoredEvent>> {
        let rows: Vec<(i64, i16, String, serde_json::Value)> = sqlx::query_as(
            "SELECT version, event_version, event_type, payload FROM events WHERE aggregate_id = $1 AND version > $2 ORDER BY version",
        )
        .bind(id.to_string())
        .bind(version)
        .fetch_all(&self.pool)
        .await
        .map_err(to_store_error)?;

        Ok(rows
            .into_iter()
            .map(
                |(version, ev_version, ev_type, payload)| upcaster::RawStoredEvent {
                    aggregate_id: id.to_string(),
                    version,
                    event_version: ev_version as u16,
                    event_type: ev_type,
                    payload,
                },
            )
            .collect())
    }
}

/// A `sqlx`-backed snapshot store for PostgreSQL.
#[derive(Debug, Clone)]
pub struct SqlxSnapshotStore<A: Aggregate> {
    pool: PgPool,
    _phantom: PhantomData<A>,
}

impl<A: Aggregate> SqlxSnapshotStore<A> {
    /// Creates a new `SqlxSnapshotStore`.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            _phantom: PhantomData,
        }
    }

    /// Ensures the `snapshots` table exists.
    #[instrument(skip(self))]
    pub async fn setup(&self) -> sqlx::Result<()> {
        sqlx::query(
            r#"
                CREATE TABLE IF NOT EXISTS snapshots (
                    aggregate_id TEXT PRIMARY KEY,
                    version BIGINT NOT NULL,
                    payload JSONB NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl<A> SnapshotStore<A> for SqlxSnapshotStore<A>
where
    A: Aggregate,
    A::Snapshot: Serialize + DeserializeOwned + Send + Sync,
    A::Id: Clone + Serialize + Send + Sync,
{
    #[instrument(skip(self, snapshot), fields(id = ?aggregate_id))]
    async fn save(&self, aggregate_id: &A::Id, version: i64, snapshot: A::Snapshot) -> Result<()> {
        let payload = serde_json::to_value(snapshot).map_err(to_serde_error)?;

        sqlx::query(
            r#"
            INSERT INTO snapshots (aggregate_id, version, payload)
            VALUES ($1, $2, $3)
            ON CONFLICT (aggregate_id) DO UPDATE
            SET version = EXCLUDED.version,
                payload = EXCLUDED.payload;
            "#,
        )
        .bind(aggregate_id.to_string())
        .bind(version)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(to_store_error)?;
        Ok(())
    }

    #[instrument(skip(self), fields(id = ?aggregate_id))]
    async fn load(&self, aggregate_id: &A::Id) -> Result<Option<StoredSnapshot<A::Snapshot>>> {
        let row: Option<(i64, serde_json::Value)> =
            sqlx::query_as("SELECT version, payload FROM snapshots WHERE aggregate_id = $1")
                .bind(aggregate_id.to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(to_store_error)?;

        match row {
            Some((version, payload)) => {
                let snapshot: A::Snapshot =
                    serde_json::from_value(payload).map_err(to_serde_error)?;
                Ok(Some(StoredSnapshot::new(
                    aggregate_id.to_string(),
                    version,
                    snapshot,
                )))
            }
            None => Ok(None),
        }
    }
}
