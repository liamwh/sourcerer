//! An in-memory snapshot store.
use std::sync::Arc;

use async_trait::async_trait;
use tracing::instrument;

use crate::{
    Aggregate, Result,
    snapshot::{SnapshotStore, StoredSnapshot},
};

use dashmap::DashMap;

/// An in-memory, thread-safe snapshot store.
///
/// This is useful for testing or for applications that do not require a
/// persistent snapshot store.
#[derive(Debug)]
pub struct InMemorySnapshotStore<A: Aggregate> {
    snapshots: Arc<DashMap<String, StoredSnapshot<A::Snapshot>>>,
}

impl<A: Aggregate> Default for InMemorySnapshotStore<A> {
    fn default() -> Self {
        Self {
            snapshots: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl<A> SnapshotStore<A> for InMemorySnapshotStore<A>
where
    A: Aggregate,
{
    #[instrument(skip(self, snapshot), fields(aggregate_id = ?aggregate_id, version))]
    async fn save(&self, aggregate_id: &A::Id, version: i64, snapshot: A::Snapshot) -> Result<()> {
        let stored_snapshot = StoredSnapshot::new(aggregate_id.to_string(), version, snapshot);
        self.snapshots
            .insert(aggregate_id.to_string(), stored_snapshot);
        Ok(())
    }

    #[instrument(skip(self), fields(aggregate_id = ?aggregate_id))]
    async fn load(&self, aggregate_id: &A::Id) -> Result<Option<StoredSnapshot<A::Snapshot>>> {
        Ok(self
            .snapshots
            .get(&aggregate_id.to_string())
            .map(|r| r.clone()))
    }
}
