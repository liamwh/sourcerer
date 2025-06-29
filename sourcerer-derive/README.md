# sourcerer-derive

Procedural macros for the [`sourcerer`](https://crates.io/crates/sourcerer) event-sourcing framework.

Currently it provides a single derive macro:

## `#[derive(Event)]`

Implements the `sourcerer::Event` trait for enums and supports the `#[event(...)]` helper attribute.

```rust
use serde::{Serialize, Deserialize};
use sourcerer_derive::Event;

#[derive(Serialize, Deserialize, Event)]
#[event(version = 2, source = "urn:my-service")]
enum AccountEvent {
    Opened,
    #[event(version = 3)]
    Credited { amount: u64 },
    #[event(source = "urn:custom")]
    Debited(u64),
}
```

The generated implementation:

* returns the variant name from `event_type()`.
* uses the configured `version`/`source`, with per-variant overrides.

Add the macro via the re-exported `derive` feature on `sourcerer`, or depend directly:

```toml
[dependencies]
sourcerer-derive = "0.1"
```

Licensed under MIT – see [LICENSE](../LICENSE).
