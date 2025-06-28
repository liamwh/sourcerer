//! An in-memory event store, useful for testing and development.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json;
use tracing::instrument;

use crate::{Aggregate, Event, EventStore, Result, StoredEvent};

use dashmap::DashMap;

// Type aliases to keep complex generic types readable and satisfy clippy::type-complexity.
type EventStream<E> = Vec<StoredEvent<E>>;

/// Thread-safe map keyed by aggregate_id
type StoreMap<E> = DashMap<String, EventStream<E>>;

/// An in-memory, thread-safe event store.
///
/// This is useful for testing or for applications that do not require a
/// persistent event store.
pub struct InMemoryEventStore<A: Aggregate> {
    events: Arc<StoreMap<A::Event>>,
}

impl<A: Aggregate> Default for InMemoryEventStore<A> {
    fn default() -> Self {
        Self {
            events: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl<A> EventStore<A> for InMemoryEventStore<A>
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

        let mut stream = self.events.entry(aggregate_id.clone()).or_default();

        let current_version = stream.last().map(|e| e.version()).unwrap_or(0);
        if current_version != expected_version {
            return Err(crate::Error::Conflict);
        }

        let mut stored_events = Vec::new();
        let mut version = current_version;
        for event in events {
            version += 1;
            let event_version = event.event_version();
            let event_type = event.event_type().to_string();
            let stored_event = StoredEvent::new(
                aggregate_id.clone(),
                version,
                event_version,
                event_type,
                event,
            );
            stream.push(stored_event.clone());
            stored_events.push(stored_event);
        }

        Ok(stored_events)
    }

    #[instrument(skip(self), fields(id = ?id))]
    async fn load(&self, id: &A::Id) -> Result<Vec<StoredEvent<A::Event>>> {
        let aggregate_id = id.to_string();

        match self.events.get(&aggregate_id) {
            Some(stream) => Ok(stream.clone()),
            None => Ok(Vec::new()),
        }
    }

    #[instrument(skip(self), fields(id = ?id, version))]
    async fn load_from(&self, id: &A::Id, version: i64) -> Result<Vec<StoredEvent<A::Event>>> {
        let aggregate_id = id.to_string();

        match self.events.get(&aggregate_id) {
            Some(stream) => Ok(stream
                .iter()
                .filter(|e| e.version() > version)
                .cloned()
                .collect()),
            None => Ok(Vec::new()),
        }
    }

    async fn load_raw(
        &self,
        id: &A::Id,
        version: i64,
    ) -> Result<Vec<crate::upcaster::RawStoredEvent>> {
        let aggregate_id = id.to_string();

        match self.events.get(&aggregate_id) {
            Some(stream) => stream
                .iter()
                .filter(|e| e.version() > version)
                .map(|e| {
                    serde_json::to_value(e.event())
                        .map_err(|se| crate::Error::Store(se.to_string()))
                        .map(|payload| crate::upcaster::RawStoredEvent {
                            aggregate_id: e.aggregate_id().to_string(),
                            version: e.version(),
                            event_version: e.event_version(),
                            event_type: e.event_type().to_string(),
                            payload,
                        })
                })
                .collect::<Result<Vec<_>>>(),
            None => Ok(Vec::new()),
        }
    }
}
