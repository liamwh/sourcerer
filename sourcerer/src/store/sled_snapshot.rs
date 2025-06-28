use std::marker::PhantomData;

use async_trait::async_trait;
use sled::Tree;
use tracing::instrument;

use crate::{
    Aggregate, Error, Result,
    snapshot::{SnapshotStore, StoredSnapshot},
};

/// A persistent, thread-safe snapshot store using `sled`.
///
/// This store uses a `sled::Tree` to store snapshots, which is an ordered
/// key-value store. Each aggregate's snapshot is stored under a key
/// corresponding to its ID.
#[derive(Debug)]
pub struct SledSnapshotStore<A: Aggregate> {
    tree: Tree,
    _phantom: PhantomData<A>,
}

impl<A: Aggregate> SledSnapshotStore<A> {
    /// Creates a new `SledSnapshotStore`.
    ///
    /// It is recommended to open a dedicated `sled::Tree` for snapshots,
    /// separate from the one used for events.
    pub fn new(tree: Tree) -> Self {
        Self {
            tree,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<A> SnapshotStore<A> for SledSnapshotStore<A>
where
    A: Aggregate,
{
    #[instrument(skip(self, snapshot), fields(aggregate_id = ?aggregate_id, version))]
    async fn save(&self, aggregate_id: &A::Id, version: i64, snapshot: A::Snapshot) -> Result<()> {
        let stored_snapshot = StoredSnapshot::new(aggregate_id.to_string(), version, snapshot);
        let value =
            serde_json::to_vec(&stored_snapshot).map_err(|e| Error::Store(e.to_string()))?;
        self.tree
            .insert(aggregate_id.to_string().as_bytes(), value)
            .map_err(|e| Error::Store(e.to_string()))?;
        Ok(())
    }

    #[instrument(skip(self), fields(aggregate_id = ?aggregate_id))]
    async fn load(&self, aggregate_id: &A::Id) -> Result<Option<StoredSnapshot<A::Snapshot>>> {
        let key = aggregate_id.to_string();
        let result = self
            .tree
            .get(key)
            .map_err(|e| Error::Store(e.to_string()))?;

        match result {
            Some(value) => {
                let snapshot =
                    serde_json::from_slice(&value).map_err(|e| Error::Store(e.to_string()))?;
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }
}
