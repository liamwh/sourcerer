//! Integration tests for Sourcerer core components.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sourcerer::{
    Aggregate, Event, EventStore, Snapshot, async_trait,
    repository::GenericRepository,
    repository::Repository,
    store::{in_memory::InMemoryEventStore, in_memory_snapshot::InMemorySnapshotStore},
};

use sourcerer::snapshot::SnapshotStore;

/// Simple event used for testing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
enum TestEvent {
    Created,
    Updated,
}

impl Event for TestEvent {
    fn event_type(&self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Updated => "Updated",
        }
    }

    fn event_version(&self) -> u16 {
        1
    }

    fn event_source(&self) -> &'static str {
        "urn:sourcerer:test"
    }
}

/// Snapshot payload for [`TestAggregate`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TestSnap {
    version: i64,
}

impl Snapshot for TestSnap {}

/// A minimal aggregate implementation used solely for testing store behaviour.
#[derive(Default, Debug)]
struct TestAggregate {
    id: Uuid,
    version: i64,
}

#[async_trait]
impl Aggregate for TestAggregate {
    type Id = Uuid;
    type Event = TestEvent;
    type Command = ();
    type Snapshot = TestSnap;
    type Error = std::convert::Infallible;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> i64 {
        self.version
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            TestEvent::Created => {}
            TestEvent::Updated => {}
        }
        self.version += 1;
    }

    async fn handle(
        &self,
        _command: Self::Command,
    ) -> std::result::Result<Vec<Self::Event>, Self::Error> {
        Ok(Vec::new())
    }

    fn from_snapshot(snapshot: Self::Snapshot) -> Self {
        Self {
            id: Uuid::new_v4(),
            version: snapshot.version,
        }
    }

    fn snapshot(&self) -> Self::Snapshot {
        TestSnap {
            version: self.version,
        }
    }
}

// -- Tests ---------------------------------------------------------------

#[test]
fn in_memory_event_store_append_and_load() {
    let store = InMemoryEventStore::<TestAggregate>::default();
    let id = Uuid::new_v4();

    // Append one event.
    let stored = futures::executor::block_on(store.append(&id, 0, vec![TestEvent::Created]))
        .expect("append should succeed");
    assert_eq!(stored.len(), 1, "one event should be stored");

    // Loading should return same event.
    let loaded = futures::executor::block_on(store.load(&id)).expect("load should succeed");
    assert_eq!(loaded.len(), 1, "one event in stream");
    assert_eq!(loaded[0].event_type(), "Created");
}

#[test]
fn in_memory_event_store_conflict() {
    let store = InMemoryEventStore::<TestAggregate>::default();
    let id = Uuid::new_v4();
    let _ = futures::executor::block_on(store.append(&id, 0, vec![TestEvent::Created]))
        .expect("initial append");

    // Appending with wrong expected_version should yield conflict.
    let err = futures::executor::block_on(store.append(&id, 0, vec![TestEvent::Updated]))
        .expect_err("should conflict");
    assert!(matches!(err, sourcerer::Error::Conflict));
}

#[test]
fn snapshot_store_save_and_load() {
    let snaps = InMemorySnapshotStore::<TestAggregate>::default();
    let id = Uuid::new_v4();

    futures::executor::block_on(snaps.save(&id, 1, TestSnap { version: 1 }))
        .expect("save snapshot");

    let loaded = futures::executor::block_on(snaps.load(&id)).expect("load");
    assert!(loaded.is_some(), "snapshot should exist");
    assert_eq!(loaded.unwrap().version(), 1);
}

#[test]
fn repository_load_and_save_with_snapshot() {
    let event_store = Arc::new(InMemoryEventStore::<TestAggregate>::default());
    let snapshot_store = Arc::new(InMemorySnapshotStore::<TestAggregate>::default());

    let repo: GenericRepository<_, _, _> =
        GenericRepository::new(event_store.clone(), Some(snapshot_store.clone()))
            .with_snapshot_frequency(Some(1));

    let id = Uuid::new_v4();

    // Create aggregate with one event.
    let mut agg = TestAggregate { id, version: 0 };
    // Apply the event locally so the aggregate's version matches optimistic locking expectations.
    let event = TestEvent::Created;
    agg.apply(&event);
    futures::executor::block_on(repo.save(&agg, vec![event])).expect("save events");

    // Loading should hydrate aggregate to version 1.
    let loaded = futures::executor::block_on(repo.load(&id)).expect("load");
    assert_eq!(loaded.version(), 1, "aggregate version after replay");

    // Snapshot should be created (frequency 1).
    let snap = futures::executor::block_on(snapshot_store.load(&id))
        .expect("load snapshot")
        .expect("snapshot exists");
    assert_eq!(snap.version(), 1);
}
