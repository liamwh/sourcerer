# Sourcerer – Event-Sourcing for Rust  🧙‍♂️

Sourcerer is a lightweight, **framework-agnostic** event-sourcing toolkit written in Rust.  It focuses on the *write side* of a typical CQRS architecture—aggregates, event stores, snapshots and up-casting—without dictating how you build projections or HTTP layers.

## ✨ Highlights

* **Pluggable stores** – In-memory (tests), `sled` (embedded) and `sqlx`-Postgres back-ends behind one trait.
* **Snapshots & up-casters** – Reduce rebuild time and evolve your event schema safely.
* **Optimistic locking** – Automatic version checks to prevent lost updates.
* **Ergonomic macros** – `#[derive(Event)]` implements boilerplate for you:

  ```rust
  use sourcerer::Event;
  use sourcerer_derive::Event; // re-exported with the "derive" feature
  use serde::{Serialize, Deserialize};

  #[derive(Serialize, Deserialize, Event)]
  #[event(version = 2, source = "urn:my-app")] // enum-level defaults
  enum AccountEvent {
      Opened,
      #[event(version = 3)]            // variant override
      Credited { amount: u64 },
      #[event(source = "urn:custom")] // another override
      Debited(u64),
  }
  ```

## 📦 Installation

```toml
[dependencies]
sourcerer = "0.1"
# Optional back-ends - disable default features when you only need one.
# default = ["in-memory"]
# features = ["sled-storage", "postgres-storage", "derive"]
```

Add `sourcerer-derive` **only if** you disabled the `derive` feature on the main crate.

```toml
sourcerer-derive = "0.1"
```

## 🚀 Quick start

```rust,no_run
use std::sync::Arc;
use uuid::Uuid;
use sourcerer::{Aggregate, async_trait};
use sourcerer::store::in_memory::{InMemoryEventStore, InMemorySnapshotStore};
use sourcerer::repository::GenericRepository;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, sourcerer_derive::Event)]
#[event(version = 1)]
enum BankEvent { Opened { balance: u64 }, Credited(u64), Debited(u64) }

#[derive(Default, Debug)]
struct Bank { id: Uuid, balance: u64, version: i64 }

#[async_trait]
impl Aggregate for Bank {
    type Id = Uuid;
    type Event = BankEvent;
    type Command = (); // left out for brevity
    type Snapshot = (); // ditto
    type Error = std::convert::Infallible;

    fn id(&self) -> &Self::Id { &self.id }
    fn version(&self) -> i64 { self.version }

    fn apply(&mut self, ev: &Self::Event) {
        match ev {
            BankEvent::Opened { balance } => self.balance = *balance,
            BankEvent::Credited(amt) => self.balance += *amt,
            BankEvent::Debited(amt) => self.balance -= *amt,
        }
        self.version += 1;
    }

    async fn handle(&self, _cmd: Self::Command) -> Result<Vec<Self::Event>, Self::Error> { Ok(vec![]) }
    fn from_snapshot(_: ()) -> Self { Self::default() }
    fn snapshot(&self) -> () {}
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let events = Arc::new(InMemoryEventStore::<Bank>::default());
    let snaps  = Arc::new(InMemorySnapshotStore::<Bank>::default());

    let repo = GenericRepository::new(events, Some(snaps));
    let bank = Bank::default();

    repo.save(&bank, vec![BankEvent::Opened { balance: 100 }]).await?;
    Ok(())
}
```

## 📚 Documentation

* Built docs: <https://docs.rs/sourcerer>
* Examples in `examples/` (to be added).

## 🛠 Feature flags

| Feature            | Default? | Description                            |
| ------------------ | -------- | -------------------------------------- |
| `in-memory`        | ✔        | Minimal, dependency-free store         |
| `sled-storage`     | ❌        | Embedded persistent store using `sled` |
| `postgres-storage` | ❌        | `sqlx`-based Postgres store            |
| `derive`           | ✔        | Re-export `sourcerer-derive`           |

Disable default features and opt-in as needed:

```toml
sourcerer = { version = "0.1", default-features = false, features = ["postgres-storage"] }
```

## 🔭 Roadmap / Ideas

* Streaming APIs (`impl Stream<Item = StoredEvent<_>>`).
* Auto-projection helpers.
* More derive macros (command helpers, snapshot versioning).

## 🤝 Contributing

1. Fork & clone.
2. `just test` (or `cargo test --all-features`).
3. Ensure `cargo clippy -D warnings` passes.

## ⚖️ License

Licensed under [MIT](LICENSE).
