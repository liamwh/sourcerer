#![allow(missing_docs)]
use serde::{Deserialize, Serialize};
use sourcerer::Event;
use sourcerer_derive::Event as DeriveEvent;

#[derive(Clone, Debug, Serialize, Deserialize, DeriveEvent)]
#[event(version = 7, source = "urn:custom")]
enum CustomEvent {
    Something,
    #[event(version = 9, source = "urn:variant")]
    Else,
}

#[test]
fn derive_macro_configurable_version_and_source() {
    assert_eq!(CustomEvent::Something.event_version(), 7);
    assert_eq!(CustomEvent::Something.event_source(), "urn:custom");
    assert_eq!(CustomEvent::Something.event_type(), "Something");

    // Variant override
    assert_eq!(CustomEvent::Else.event_version(), 9);
    assert_eq!(CustomEvent::Else.event_source(), "urn:variant");
    assert_eq!(CustomEvent::Else.event_type(), "Else");
}
