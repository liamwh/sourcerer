//! CloudEvent conversion utilities.
//!
//! This module provides a lightweight [`CloudEvent`] newtype that wraps a
//! [`cloudevents_sdk::Event`] and a blanket `From` implementation which turns
//! any `Event` from this crate into a CloudEvent.
//!
//! # Example
//!
//! ```rust
//! # use uuid::Uuid;
//! use sourcerer::{Event as _, cloudevent::CloudEvent};
//! # use serde::{Serialize, Deserialize};
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MyEvent;
//! impl sourcerer::Event for MyEvent {
//!     fn event_type(&self) -> &'static str { "MyEvent" }
//!     fn event_version(&self) -> u16 { 1 }
//!     fn event_source(&self) -> &'static str { "urn:sourcerer:test" }
//! }
//! # let my_event = MyEvent;
//! let ce: CloudEvent = my_event.into();
//! ```
//!
//! A random UUID is generated for the CloudEvent `id` field and the `source`
//! attribute defaults to `"urn:sourcerer:event"`. If you need more control
//! build the underlying event manually via the `into_inner` method.

use crate::{Error, Event, Result};
use cloudevents::event::{Data, Event as CeEvent, EventBuilder, EventBuilderV10};
use serde::Serialize;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

/// Newtype wrapper around `cloudevents_sdk::Event` so we can legally provide a
/// blanket [`From`] implementation without violating Rust's orphan rules.
#[derive(Debug, Clone)]
pub struct CloudEvent(pub CeEvent);

impl CloudEvent {
    /// Returns the inner [`cloudevents_sdk::Event`].
    #[must_use]
    pub fn into_inner(self) -> CeEvent {
        self.0
    }

    /// Builds a [`CloudEvent`] from an `Event` and an explicit [`Url`] source.
    #[instrument(skip(event))]
    pub fn from_event_with_source<E>(event: E, source: Url) -> Result<Self>
    where
        E: Event + Serialize,
    {
        let id = Uuid::new_v4().to_string();

        let data_json = serde_json::to_vec(&event)
            .map_err(|e| Error::Validation(format!("failed to serialise event: {e}")))?;

        let ce = EventBuilderV10::new()
            .id(id)
            .ty(event.event_type())
            .source(source)
            .data("application/json", Data::from(data_json))
            .build()
            .map_err(|e| Error::Validation(format!("failed to build CloudEvent: {e}")))?;

        Ok(Self(ce))
    }
}

impl<E> From<E> for CloudEvent
where
    E: Event + Serialize,
{
    fn from(event: E) -> Self {
        let source_str = event.event_source();
        let source = Url::parse(source_str)
            .unwrap_or_else(|_| Url::parse("urn:sourcerer:event").expect("default URN is valid"));

        // Safe unwrap: if both parses failed we'd have panicked above.
        Self::from_event_with_source(event, source).expect("constructing CloudEvent cannot fail")
    }
}
