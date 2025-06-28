//! Provides a generic repository for interacting with aggregates.
use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use tracing::instrument;

use crate::{
    Aggregate, Error, EventStore, Result, snapshot::SnapshotStore, upcaster::UpcasterChain,
};

/// Defines the standard interface for a repository.
#[async_trait]
pub trait Repository<A: Aggregate>: Send + Sync {
    /// Loads an aggregate instance from the store.
    async fn load(&self, id: &A::Id) -> Result<A>;
    /// Saves a new list of events for an aggregate.
    async fn save(&self, aggregate: &A, new_events: Vec<A::Event>) -> Result<()>;
}

/// A generic, high-level repository for loading and saving aggregates.
///
/// This repository simplifies the common load-handle-save cycle by
/// orchestrating the `EventStore` and an optional `SnapshotStore`.
pub struct GenericRepository<A, S, SS>
where
    A: Aggregate,
    S: EventStore<A>,
    SS: SnapshotStore<A>,
{
    store: Arc<S>,
    snapshot_store: Option<Arc<SS>>,
    upcasters: UpcasterChain<A::Event>,
    snapshot_frequency: Option<usize>,
    _phantom: PhantomData<A>,
}

impl<A, S, SS> GenericRepository<A, S, SS>
where
    A: Aggregate,
    S: EventStore<A>,
    SS: SnapshotStore<A>,
{
    /// Creates a new `GenericRepository`.
    pub fn new(store: Arc<S>, snapshot_store: Option<Arc<SS>>) -> Self {
        Self {
            store,
            snapshot_store,
            upcasters: UpcasterChain::new(),
            snapshot_frequency: None,
            _phantom: PhantomData,
        }
    }

    /// Sets the upcaster chain for the repository.
    pub fn with_upcasters(mut self, upcasters: UpcasterChain<A::Event>) -> Self {
        self.upcasters = upcasters;
        self
    }

    /// Sets the frequency at which snapshots should be created.
    ///
    /// For example, a value of `Some(100)` means a snapshot will be created
    /// every 100 events.
    pub fn with_snapshot_frequency(mut self, frequency: Option<usize>) -> Self {
        self.snapshot_frequency = frequency;
        self
    }
}

#[async_trait]
impl<A, S, SS> Repository<A> for GenericRepository<A, S, SS>
where
    A: Aggregate,
    S: EventStore<A> + 'static,
    SS: SnapshotStore<A> + 'static,
{
    #[instrument(skip(self), fields(aggregate.id = ?id))]
    async fn load(&self, id: &A::Id) -> Result<A> {
        // Attempt to hydrate the aggregate from a snapshot first so we can
        // replay only the delta of events that occurred afterwards.
        let (mut aggregate, starting_version, has_snapshot) =
            if let Some(snapshot_store) = &self.snapshot_store {
                if let Some(stored) = snapshot_store.load(id).await? {
                    let v = stored.version();
                    let snap = stored.into_snapshot();
                    (A::from_snapshot(snap), v, true)
                } else {
                    (A::default(), 0, false)
                }
            } else {
                (A::default(), 0, false)
            };

        // Load all events that occurred after the snapshot (or from scratch).
        let raw_events = self.store.load_raw(id, starting_version).await?;

        // Guard against loading a non-existing aggregate.
        if raw_events.is_empty() && !has_snapshot {
            return Err(Error::NotFound);
        }

        for raw_event in raw_events {
            let upcasted_event = self.upcasters.upcast(raw_event)?;
            let event = serde_json::from_value(upcasted_event.payload)
                .map_err(|e| Error::Store(e.to_string()))?;
            aggregate.apply(&event);
        }

        Ok(aggregate)
    }

    #[instrument(skip(self, aggregate, new_events), fields(aggregate.id = ?aggregate.id()))]
    async fn save(&self, aggregate: &A, new_events: Vec<A::Event>) -> Result<()> {
        if new_events.is_empty() {
            return Ok(());
        }

        let version_before_save = aggregate.version() - new_events.len() as i64;
        let num_new_events = new_events.len() as i64;

        self.store
            .append(aggregate.id(), version_before_save, new_events)
            .await?;

        if let (Some(snapshot_store), Some(frequency)) =
            (&self.snapshot_store, self.snapshot_frequency)
        {
            let version_after_save = version_before_save + num_new_events;
            if version_after_save / frequency as i64 > version_before_save / frequency as i64 {
                let snapshot = aggregate.snapshot();
                snapshot_store
                    .save(aggregate.id(), version_after_save, snapshot)
                    .await?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<A, R> Repository<A> for Arc<R>
where
    A: Aggregate,
    R: Repository<A> + Send + Sync,
{
    async fn load(&self, aggregate_id: &A::Id) -> Result<A> {
        (**self).load(aggregate_id).await
    }

    async fn save(&self, aggregate: &A, events: Vec<A::Event>) -> Result<()> {
        (**self).save(aggregate, events).await
    }
}
