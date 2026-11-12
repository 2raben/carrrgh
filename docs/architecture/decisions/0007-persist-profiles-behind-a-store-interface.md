---
tags: [adr]
status: accepted
date: 2026-11-12
sources:
  - src/storage.rs
  - src/storage/sqlite.rs
  - src/storage/memory.rs
  - src/profile.rs
  - src/run.rs
  - src/main.rs
  - Cargo.toml
  - ../../pitches/coins/pitch--coins-and-player-profile.md
---

# 0007 — Persist profiles in SQLite behind an asynchronous store interface

## Context

The [coins and player profile pitch](../../pitches/coins/pitch--coins-and-player-profile.md)
introduces the first data that must outlive a drive and the process: a player
profile with a coin balance. Until now the game had no persistence, no new
dependency beyond `fastrand`, and a simulation that runs headless in tests.

Native builds can write files; a possible browser build cannot, and its storage
APIs (IndexedDB, `localStorage`) are asynchronous or origin-scoped. Alternatives
considered while shaping: serialising the ECS world with `DynamicWorldBuilder`
(no stable IDs, migrations or transactional writes), a hand-written file format
with atomic rename (needs its own transaction and recovery logic), Bevy's
`SettingsPlugin` (intended for preferences, not game data) and SQLite.

## Decision

Game code talks only to the `ProfileStore` trait in [storage.rs](../../../src/storage.rs):
load the first profile, create a profile, credit a drive's coins. It exchanges
plain data types (`Profile`, `PlayerName`, `RunCredit`), never ECS components or
SQL rows, and returns `BoxedFuture`s so native and browser backends fit the same
contract. [profile.rs](../../../src/profile.rs) runs one operation at a time on
Bevy's `IoTaskPool` and applies the result in `Update`; nothing in the fixed step
waits for storage. Only on `AppExit` does it block, to write outstanding credits.

The native backend in [storage/sqlite.rs](../../../src/storage/sqlite.rs) uses
`rusqlite` 0.41 with bundled SQLite, so no system library is required. It stores one
file, `profiles.sqlite3`, in the platform data directory from `dirs` 7.0, or in
`$CAR_DRIVE_DATA_DIR`. Both crates are native-only dependencies. The file carries
an `application_id` and `user_version` (schema 1). Empty files are initialised;
foreign, newer or non-database files are refused without writing. Each credit is
an `IMMEDIATE` transaction that records the run id and adds the coins, so a
retried credit counts once. Stored names and balances are validated on load.

Tests use the in-memory store, whose futures complete when first polled, so
headless tests stay deterministic; SQLite is tested directly and once through the
plugin with a bounded wait.

## Consequences

- Coins survive game over, restart and quitting. A crash loses at most the drive
  in progress.
- Bundled SQLite adds a C compile step (the project's Clang is enough) and some
  binary size. Dependency updates stay deliberate through `Cargo.lock`.
- A browser build needs another `ProfileStore` implementation, for example on
  IndexedDB; SQLite is not compiled for `wasm32`. The trait and the profile
  plugin need no changes for that, but the browser backend must answer
  asynchronously and confirm commits before resolving.
- Schema changes need a numbered migration step and a test with a file of the
  previous version.
- The save file is not protected against editing; this is acceptable for a local
  single-player game without leaderboards.

## The tell

`rg -n "rusqlite|Connection" src` should match only `src/storage/sqlite.rs`. A
match elsewhere means SQLite leaked through the interface. The test
`sqlite_profile_and_coins_survive_restarting_the_game` covers the native path end
to end; it cannot show that a future browser backend honours the same guarantees.
