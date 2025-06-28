//! Defines the upcasting mechanism for handling event schema versioning.
use serde_json::Value;

use crate::{Event, Result};

/// A raw, stored event, used for upcasting before deserialization.
#[derive(Debug)]
pub struct RawStoredEvent {
    /// The ID of the aggregate this event belongs to.
    pub aggregate_id: String,
    /// The version of the aggregate after this event was applied.
    pub version: i64,
    /// The version of the event's schema.
    pub event_version: u16,
    /// The type of the event.
    pub event_type: String,
    /// The event payload itself.
    pub payload: Value,
}

/// Defines the interface for an upcaster.
///
/// An upcaster is responsible for transforming an event from an older version
/// to a newer one. This is a key part of handling schema evolution.
pub trait Upcaster<E: Event>: Send + Sync {
    /// The type of event this upcaster can handle.
    fn event_type(&self) -> &'static str;

    /// The version of the event this upcaster can transform from.
    fn source_version(&self) -> u16;

    /// The version of the event this upcaster transforms to.
    fn target_version(&self) -> u16 {
        self.source_version() + 1
    }

    /// Transforms a JSON payload of an event into its next version.
    fn upcast(&self, payload: Value) -> Result<Value>;
}

/// A chain of upcasters that can be applied sequentially to an event.
pub struct UpcasterChain<E: Event> {
    upcasters: Vec<Box<dyn Upcaster<E>>>,
}

impl<E: Event> Default for UpcasterChain<E> {
    fn default() -> Self {
        Self {
            upcasters: Vec::new(),
        }
    }
}

impl<E: Event> UpcasterChain<E> {
    /// Creates a new, empty upcaster chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an upcaster to the chain.
    pub fn with<U: Upcaster<E> + 'static>(mut self, upcaster: U) -> Self {
        self.upcasters.push(Box::new(upcaster));
        self
    }

    /// Applies the upcasting chain to a raw stored event.
    ///
    /// It will continue to apply upcasters until the event's version matches
    /// the latest version known to the application.
    pub(crate) fn upcast(&self, event: RawStoredEvent) -> Result<RawStoredEvent> {
        let mut current_version = event.event_version;
        let mut payload = event.payload;
        let event_type = event.event_type.clone();

        while let Some(upcaster) = self
            .upcasters
            .iter()
            .find(|u| u.event_type() == event_type && u.source_version() == current_version)
        {
            payload = upcaster.upcast(payload)?;
            current_version = upcaster.target_version();
        }

        Ok(RawStoredEvent {
            aggregate_id: event.aggregate_id,
            version: event.version,
            event_version: current_version,
            event_type,
            payload,
        })
    }
}
