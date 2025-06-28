//! A persistent `EventStore` and `SnapshotStore` implementation using `sled`.

use std::marker::PhantomData;

use async_trait::async_trait;
use serde_json;
use tracing::instrument;

use crate::{Aggregate, Error, Event, EventStore, Result, StoredEvent};

/// A persistent, thread-safe event store using `sled`.
///
/// This store uses a `sled::Tree` to store events, which is an ordered
/// key-value store. This allows for efficient scanning of event streams.
#[derive(Clone)]
pub struct SledEventStore<A: Aggregate> {
    db: sled::Db,
    _phantom: PhantomData<A>,
}

impl<A: Aggregate> SledEventStore<A> {
    /// Creates a new `SledEventStore`.
    pub fn new(db: sled::Db) -> Self {
        Self {
            db,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<A> EventStore<A> for SledEventStore<A>
where
    A: Aggregate,
{
    #[instrument(skip(self, events), fields(id = ?id, expected_version))]
    async fn append(
        &self,
        id: &A::Id,
        expected_version: i64,
        events: Vec<A::Event>,
    ) -> Result<Vec<StoredEvent<A::Event>>> {
        let aggregate_id = id.to_string();
        let tree = self
            .db
            .open_tree(aggregate_id.as_bytes())
            .map_err(|e| Error::Store(e.to_string()))?;

        let current_version = match tree.last() {
            Ok(Some((_, v))) => {
                let e: StoredEvent<A::Event> =
                    serde_json::from_slice(&v).map_err(|e| Error::Store(e.to_string()))?;
                e.version()
            }
            _ => 0,
        };

        if current_version != expected_version {
            return Err(crate::Error::Conflict);
        }

        let event_types: Vec<String> = events.iter().map(|e| e.event_type().to_string()).collect();
        let num_events = events.len();

        let mut stored_events = Vec::new();
        let mut events_to_commit = Vec::new();

        for (event, (version, event_type)) in events.into_iter().zip(
            (1..=num_events as i64)
                .map(|i| expected_version + i)
                .zip(event_types.into_iter()),
        ) {
            let stored_event = StoredEvent::new(
                aggregate_id.clone(),
                version,
                event.event_version(),
                event_type,
                event,
            );
            let value =
                serde_json::to_vec(&stored_event).map_err(|e| Error::Store(e.to_string()))?;
            stored_events.push(stored_event.clone());
            let key = format!("{aggregate_id}/{version}");
            events_to_commit.push((key, value));
        }

        tree.transaction(|tx| {
            for (key, value) in &events_to_commit {
                tx.insert(key.as_bytes(), value.as_slice())?;
            }
            Ok(())
        })
        .map_err(|e: sled::transaction::TransactionError| Error::Store(e.to_string()))?;

        Ok(stored_events)
    }

    #[instrument(skip(self), fields(id = ?id))]
    async fn load(&self, id: &A::Id) -> Result<Vec<StoredEvent<A::Event>>> {
        let aggregate_id = id.to_string();
        let tree = self
            .db
            .open_tree(aggregate_id.as_bytes())
            .map_err(|e| Error::Store(e.to_string()))?;
        let prefix = format!("{aggregate_id}/");

        tree.scan_prefix(prefix.as_bytes())
            .map(|res| {
                let (_, v) = res.map_err(|e| Error::Store(e.to_string()))?;
                serde_json::from_slice(&v).map_err(|e| Error::Store(e.to_string()))
            })
            .collect()
    }

    #[instrument(skip(self), fields(id = ?id, version))]
    async fn load_from(&self, id: &A::Id, version: i64) -> Result<Vec<StoredEvent<A::Event>>> {
        let aggregate_id = id.to_string();
        let tree = self
            .db
            .open_tree(aggregate_id.as_bytes())
            .map_err(|e| Error::Store(e.to_string()))?;
        let start_key = format!("{aggregate_id}/{}", version + 1);

        tree.range(start_key.as_bytes()..)
            .map(|res| {
                let (_, v) = res.map_err(|e| Error::Store(e.to_string()))?;
                serde_json::from_slice(&v).map_err(|e| Error::Store(e.to_string()))
            })
            .collect()
    }

    async fn load_raw(
        &self,
        id: &<A as Aggregate>::Id,
        version: i64,
    ) -> Result<Vec<crate::upcaster::RawStoredEvent>> {
        let aggregate_id = id.to_string();
        let tree = self
            .db
            .open_tree(aggregate_id.as_bytes())
            .map_err(|e| Error::Store(e.to_string()))?;
        let start_key = format!("{aggregate_id}/{}", version + 1);

        tree.range(start_key.as_bytes()..)
            .map(|res| {
                let (_, v) = res.map_err(|e| Error::Store(e.to_string()))?;
                let stored: StoredEvent<A::Event> =
                    serde_json::from_slice(&v).map_err(|e| Error::Store(e.to_string()))?;
                let payload = serde_json::to_value(stored.event())
                    .map_err(|e| Error::Store(e.to_string()))?;
                Ok(crate::upcaster::RawStoredEvent {
                    aggregate_id: stored.aggregate_id().to_string(),
                    version: stored.version(),
                    event_version: stored.event_version(),
                    event_type: stored.event_type().to_string(),
                    payload,
                })
            })
            .collect()
    }
}
