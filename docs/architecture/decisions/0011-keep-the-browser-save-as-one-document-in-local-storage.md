---
tags: [adr]
status: accepted
date: 2026-11-14
amends: [0007-persist-profiles-behind-a-store-interface.md]
sources:
  - src/storage/document.rs
  - src/storage/browser.rs
  - src/storage/memory.rs
  - src/storage/contract.rs
  - src/main.rs
  - Cargo.toml
  - ../../pitches/browser-game/pitch--browser-game.md
---

# 0011 — Keep the browser save as one document in `localStorage`

## Context

The [browser game pitch](../../pitches/browser-game/pitch--browser-game.md) builds
Car Drive for `wasm32` and publishes it on GitHub Pages. `SqliteStore`
([ADR 0007](0007-persist-profiles-behind-a-store-interface.md)) is native only, and
the garage rules of [ADR 0008](0008-transact-vehicle-ownership-and-tuning.md) are
written as SQL transactions, so the browser needs its own `ProfileStore` and its own
copy of those rules. The private network market of
[ADR 0009](0009-verify-drives-in-a-private-market-service.md) uses raw TCP, which a
browser cannot open.

Alternatives considered:

- **IndexedDB object stores.** Real asynchronous transactions and more room, but
  its JavaScript handles are not `Send`, so they clash with `BoxedFuture` and
  `IoTaskPool`, and it needs `wasm-bindgen-futures` and more `web-sys` glue.
- **SQLite compiled to wasm** behind an IndexedDB or OPFS file system. It would
  reuse the SQL, but makes the page larger and OPFS needs a worker.
- **One JSON document in `localStorage`.** Synchronous and small. A single
  `setItem` replaces the whole save, so each operation is atomic.

## Decision

[storage/document.rs](../../../src/storage/document.rs) holds the whole save as
`SaveDocument`: profiles, the most recent credited run IDs, every vehicle with its
owner or none for a listing, garages with the active car and the starter grant,
and the most recent garage receipts. Its format marker and version
(`car-drive-save`, 1) are independent of the SQLite schema. Decoding refuses texts
over 2 MiB and unknown fields. A newer version is reported as `NewerSchema`, and
everything else is validated as SQLite validates on load: names, balances up to
`i64::MAX`, vehicles, owners, and an owned active car.

Garage operations mirror `sqlite/garage.rs::transact`. They are applied through
`apply` to a decoded copy, which returns the new text only when something
changed. An error drops the copy, like a rollback. Receipts and credited runs keep
only the latest 256 entries. They protect retries of the operation in flight, and
the client allows only one at a time. Native SQLite keeps all of them.

`DocumentStore<S: Slot>` implements `ProfileStore` over that document, and every
operation finishes before its future is returned:

- [storage/browser.rs](../../../src/storage/browser.rs): `BrowserStore`, wasm only,
  uses the `localStorage` item `car-drive/save`. It fetches `web_sys::Storage` for
  each call and keeps no browser object. An invalid save moves to
  `car-drive/save.broken` and the game starts a new one. A newer save is refused
  and left untouched.
- [storage/memory.rs](../../../src/storage/memory.rs): `MemoryStore`, in tests
  only, keeps the encoded text in memory and now supports the garage.

[storage/contract.rs](../../../src/storage/contract.rs) runs the same garage and
credit scenarios against `SqliteStore` and `MemoryStore`. [main.rs](../../../src/main.rs)
chooses the store by target. `market.rs`, `SqliteStore` and drive verification
compile natively only, so the browser has a local market and no network store.
`fastrand` gets its `js` feature on wasm so seeds and IDs come from the browser's
crypto.

## Consequences

- Native saves, the SQLite schema and the private market are unchanged.
- The garage rules exist twice. The contract tests catch drift in a normal native
  `cargo test`, but only for the scenarios they cover.
- The browser keeps one save per browser profile and origin. On GitHub Pages all
  of the account's project pages share that origin, including Moto Devil. The
  `car-drive/` prefix avoids collisions. Other pages could still edit the save,
  which is accepted, since local coins are already editable (ADR 0007).
- Two tabs read the save afresh for each operation; writes at the same moment
  are settled by the last one winning. A blocked or full `localStorage` reports
  `Unavailable`, and the game plays on without saving.
- Closing the tab sends no `AppExit`. Credits are already written when they are
  issued, so only a drive in progress is lost.

## The tell

- `cargo test --locked storage::` runs `storage::contract::{sqlite,document}::*`
  and the document tests: edited, newer and foreign saves, no write after a
  failure, and bounded receipts.
- `rg -n "web_sys" src` should match only `src/storage/browser.rs`.
- `cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings` must stay
  clean.
- The `localStorage` glue has no automated test. Check it in a browser: create a
  player, drive, buy, sell, reload, and the save is still there. The browser's
  storage inspector shows `car-drive/save`.
