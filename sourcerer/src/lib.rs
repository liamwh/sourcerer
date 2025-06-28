//! # Event Sourcing Framework
//!
//! `sourcerer` is a Rust framework for building event-sourced applications.
//! It provides the core traits and components to get started with event
//! sourcing, including aggregates, events, event stores, and repositories.
//!
//! ## Core Concepts
//!
//! - **[`Aggregate`]**: A consistency boundary that processes commands and
//!   produces events.
//! - **[`Event`]**: An immutable fact that represents a change in the state of
//!   an aggregate.
//! - **[`EventStore`]**: A persistent store for events.
//! - **[`SnapshotStore`]**: A persistent store for aggregate snapshots, used to
//!   optimize loading.
//! - **[`Repository`]**: A high-level API for loading aggregates, handling
//!   commands, and saving events.
//!
//! ## Example
//!
//! ```rust,no_run
//! // 1. Define your aggregate, events, and commands.
//! use sourcerer::{Aggregate, AggregateId, Event, Snapshot};
//! use serde::{Deserialize, Serialize};
//! use uuid::Uuid;
//!
//! #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
//! pub enum BankAccountEvent {
//!     Opened { initial_balance: u64 },
//!     Credited { amount: u64 },
//!     Debited { amount: u64 },
//! }
//! impl Event for BankAccountEvent {
//!    fn event_type(&self) -> &'static str {
//!        match self {
//!            BankAccountEvent::Opened { .. } => "Opened",
//!            BankAccountEvent::Credited { .. } => "Credited",
//!            BankAccountEvent::Debited { .. } => "Debited",
//!        }
//!    }
//!    fn event_version(&self) -> u16 {
//!        1
//!    }
//!    fn event_source(&self) -> &'static str { "urn:sourcerer:bank" }
//! }
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct BankAccountSnapshot {
//!     balance: u64,
//! }
//! impl Snapshot for BankAccountSnapshot {}
//!
//! #[derive(Debug)]
//! pub enum BankAccountCommand {
//!     Open { initial_balance: u64 },
//!     Deposit { amount: u64 },
//!     Withdraw { amount: u64 },
//! }
//!
//! #[derive(Debug, Default)]
//! pub struct BankAccount {
//!     id: Uuid,
//!     balance: u64,
//!     version: i64,
//! }
//!
//! // 2. Implement the Aggregate trait.
//! #[sourcerer::async_trait]
//! impl Aggregate for BankAccount {
//!     type Id = Uuid;
//!     type Event = BankAccountEvent;
//!     type Command = BankAccountCommand;
//!     type Snapshot = BankAccountSnapshot;
//!     type Error = std::convert::Infallible;
//!
//!     fn id(&self) -> &Self::Id {
//!         &self.id
//!     }
//!
//!     fn version(&self) -> i64 {
//!         self.version
//!     }
//!
//!     fn apply(&mut self, event: &Self::Event) {
//!         match event {
//!             BankAccountEvent::Opened { initial_balance } => {
//!                 self.balance = *initial_balance;
//!                 self.id = Uuid::new_v4();
//!             }
//!             BankAccountEvent::Credited { amount } => {
//!                 self.balance += *amount;
//!             }
//!             BankAccountEvent::Debited { amount } => {
//!                 self.balance -= *amount;
//!             }
//!         }
//!         self.version += 1;
//!     }
//!
//!     async fn handle(&self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
//!         // Business logic and validation here...
//!         Ok(vec![])
//!     }
//!
//!     fn from_snapshot(snapshot: Self::Snapshot) -> Self {
//!         Self {
//!             balance: snapshot.balance,
//!             ..Default::default()
//!         }
//!     }
//!
//!     fn snapshot(&self) -> Self::Snapshot {
//!         BankAccountSnapshot {
//!             balance: self.balance
//!         }
//!     }
//! }
//!
//! // 3. Use the repository to interact with your aggregate.
//! use sourcerer::store::in_memory::InMemoryEventStore;
//! use sourcerer::store::in_memory_snapshot::InMemorySnapshotStore;
//! use sourcerer::repository::GenericRepository;
//! use std::sync::Arc;
//!
//! async fn bank_account_example() {
//!     let event_store = Arc::new(InMemoryEventStore::<BankAccount>::default());
//!     let snapshot_store = Arc::new(InMemorySnapshotStore::<BankAccount>::default());
//!     let repo = GenericRepository::new(event_store, Some(snapshot_store));
//!
//!     // ...
//! }
//! ```
#![deny(missing_docs)]

use std::fmt::Debug;

pub use async_trait::async_trait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub mod cloudevent;
pub mod repository;
pub mod snapshot;
pub mod store;
pub mod upcaster;

pub use cloudevent::CloudEvent;

/// The error type for this crate.
#[derive(Debug, thiserror::Error, Clone)]
pub enum Error {
    /// Occurs when an aggregate's expected version does not match the actual
    /// version, indicating a concurrency conflict.
    #[error("aggregate conflict")]
    Conflict,
    /// Occurs when an aggregate could not be found.
    #[error("aggregate not found")]
    NotFound,
    /// Wraps an error from the underlying event or snapshot store.
    #[error("event store error: {0}")]
    Store(String),
    /// Occurs when a command fails a validation rule.
    #[error("validation error: {0}")]
    Validation(String),
}

/// A specialized `Result` type for this crate's operations.
pub type Result<T> = std::result::Result<T, Error>;

/// A marker trait for events.
///
/// Events must be serializable, deserializable, clonable, and debuggable.
/// The `Event` derive macro can be used to automatically implement this trait.
pub trait Event: Serialize + DeserializeOwned + Clone + Debug + Send + Sync {
    /// Returns a static string slice representing the type of the event.
    fn event_type(&self) -> &'static str;

    /// Returns the version of the event's schema.
    fn event_version(&self) -> u16;

    /// Returns the CloudEvent `source` URI associated with this event.
    ///
    /// By default this returns `"urn:sourcerer:event"`. Override this in your
    /// event types if you need a different source.
    fn event_source(&self) -> &'static str;
}

/// Uniquely identifies an aggregate instance.
pub trait AggregateId:
    Eq + std::hash::Hash + Clone + Send + Sync + ToString + Debug + std::fmt::Display + 'static
{
    /// Creates a new, unique aggregate ID.
    fn new() -> Self;
}

impl AggregateId for Uuid {
    fn new() -> Self {
        Uuid::new_v4()
    }
}

/// An aggregate is a consistency boundary. It is the fundamental building block
/// of the domain model.
#[async_trait]
pub trait Aggregate: Default + Send + Sync + 'static {
    /// The type of the aggregate's unique identifier.
    type Id: AggregateId;
    /// The type of events that this aggregate produces.
    type Event: Event;
    /// The type of commands that this aggregate can handle.
    type Command: Debug;
    /// The type of snapshot that this aggregate can produce.
    type Snapshot: Snapshot;
    /// The type of error that this aggregate can produce.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Returns the unique identifier of the aggregate.
    fn id(&self) -> &Self::Id;

    /// Returns the current version of the aggregate.
    fn version(&self) -> i64;

    /// Applies an event to the aggregate, changing its state.
    fn apply(&mut self, event: &Self::Event);

    /// Handles a command and returns a list of events.
    async fn handle(
        &self,
        command: Self::Command,
    ) -> std::result::Result<Vec<Self::Event>, Self::Error>;

    /// Restores the aggregate's state from a snapshot.
    fn from_snapshot(snapshot: Self::Snapshot) -> Self;

    /// Creates a snapshot of the aggregate's current state.
    fn snapshot(&self) -> Self::Snapshot;

    /// Restores the aggregate's state from a sequence of events.
    fn load<E: Into<Self::Event>, I: IntoIterator<Item = E>>(events: I) -> Self {
        let mut aggregate = Self::default();
        for event in events {
            aggregate.apply(&event.into());
        }
        aggregate
    }
}

/// A marker trait for snapshots.
pub trait Snapshot: Serialize + DeserializeOwned + Clone + Debug + Send + Sync {}

/// Represents a stored event, including metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "E: Serialize",
    deserialize = "E: serde::de::DeserializeOwned"
))]
pub struct StoredEvent<E: Event> {
    /// The ID of the aggregate this event belongs to.
    aggregate_id: String,
    /// The version of the aggregate after this event was applied.
    version: i64,
    /// The version of the event's schema.
    event_version: u16,
    /// The type of the event.
    event_type: String,
    /// The event payload itself.
    event: E,
}

impl<E: Event> StoredEvent<E> {
    /// Creates a new stored event.
    pub fn new(
        aggregate_id: String,
        version: i64,
        event_version: u16,
        event_type: String,
        event: E,
    ) -> Self {
        Self {
            aggregate_id,
            version,
            event_version,
            event_type,
            event,
        }
    }

    /// Returns the ID of the aggregate this event belongs to.
    pub fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }
    /// Returns the version of the aggregate after this event was applied.
    pub fn version(&self) -> i64 {
        self.version
    }
    /// Returns the version of the event's schema.
    pub fn event_version(&self) -> u16 {
        self.event_version
    }
    /// Returns the type of the event.
    pub fn event_type(&self) -> &str {
        &self.event_type
    }
    /// Returns the event payload itself.
    pub fn event(&self) -> &E {
        &self.event
    }
    /// Consumes the stored event and returns the event payload.
    pub fn into_event(self) -> E {
        self.event
    }
}

/// The trait for event stores.
#[async_trait]
pub trait EventStore<A: Aggregate>: Send + Sync {
    /// Appends a list of events to the event store for a given aggregate.
    ///
    /// This operation must be atomic. It should fail if the `expected_version`
    /// does not match the current version of the aggregate, preventing
    /// optimistic concurrency conflicts.
    async fn append(
        &self,
        id: &A::Id,
        expected_version: i64,
        events: Vec<A::Event>,
    ) -> Result<Vec<StoredEvent<A::Event>>>;

    /// Loads the full event stream for a given aggregate.
    async fn load(&self, id: &A::Id) -> Result<Vec<StoredEvent<A::Event>>>;

    /// Loads the event stream for a given aggregate starting from a specific
    /// version. This is used to hydrate an aggregate after loading it from
    /// a snapshot.
    async fn load_from(&self, id: &A::Id, version: i64) -> Result<Vec<StoredEvent<A::Event>>>;

    /// Loads the raw event stream for a given aggregate.
    ///
    /// This is used by the `GenericRepository` to perform upcasting before
    /// deserializing the events.
    async fn load_raw(
        &self,
        id: &A::Id,
        version: i64,
    ) -> Result<Vec<crate::upcaster::RawStoredEvent>>;
}
