//! The snapshot module contains the traits and structs for creating and storing
//! aggregate snapshots.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{Aggregate, Result, Snapshot};

/// Represents a stored snapshot, including metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize",
    deserialize = "S: serde::de::DeserializeOwned"
))]
pub struct StoredSnapshot<S: Snapshot> {
    /// The ID of the aggregate this snapshot belongs to.
    aggregate_id: String,
    /// The version of the aggregate when this snapshot was taken.
    version: i64,
    /// The snapshot payload itself.
    snapshot: S,
}

impl<S: Snapshot> StoredSnapshot<S> {
    /// Creates a new stored snapshot.
    pub fn new(aggregate_id: String, version: i64, snapshot: S) -> Self {
        Self {
            aggregate_id,
            version,
            snapshot,
        }
    }

    /// Returns the aggregate ID.
    pub fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    /// Returns the version of the aggregate when this snapshot was taken.
    pub fn version(&self) -> i64 {
        self.version
    }

    /// Consumes the stored snapshot and returns the inner snapshot.
    pub fn into_snapshot(self) -> S {
        self.snapshot
    }
}

/// A snapshot store is responsible for persisting and loading snapshots.
///
/// Snapshots are an optimization to reduce the time it takes to hydrate an
/// aggregate. Instead of replaying all events from the beginning of time, an
/// aggregate can be restored from a recent snapshot and then only replay the
/// events that occurred after it.
#[async_trait]
pub trait SnapshotStore<A: Aggregate>: Send + Sync {
    /// Saves a snapshot for a given aggregate.
    ///
    /// This should overwrite any existing snapshot for the same aggregate.
    async fn save(&self, aggregate_id: &A::Id, version: i64, snapshot: A::Snapshot) -> Result<()>;

    /// Loads the latest snapshot for a given aggregate.
    async fn load(&self, aggregate_id: &A::Id) -> Result<Option<StoredSnapshot<A::Snapshot>>>;
}
