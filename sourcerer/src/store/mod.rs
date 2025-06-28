//! The store module contains the implementations of the event and snapshot
//! stores.

// The in-memory implementations are compiled when the `in-memory` feature is
// enabled (this is the default).
#[cfg(feature = "in-memory")]
/// An in-memory event store.
pub mod in_memory;

#[cfg(feature = "in-memory")]
/// An in-memory snapshot store.
pub mod in_memory_snapshot;

// The persistent `sled` implementations are compiled when the `sled-storage`
// feature is enabled.
#[cfg(feature = "sled-storage")]
/// A persistent event store using `sled`.
pub mod sled;

#[cfg(feature = "sled-storage")]
/// A persistent snapshot store using `sled`.
pub mod sled_snapshot;

// SQLx / Postgres implementation compiled when the `postgres-storage` feature
// is enabled.
#[cfg(feature = "postgres-storage")]
pub mod sqlx_postgres;
