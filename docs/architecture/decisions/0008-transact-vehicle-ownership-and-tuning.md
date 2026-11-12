---
tags: [adr]
status: accepted
date: 2026-11-12
amends: [0007-persist-profiles-behind-a-store-interface.md]
sources:
  - src/vehicles.rs
  - src/garage.rs
  - src/storage/sqlite/garage.rs
  - src/vehicle_art.rs
  - ../../pitches/vehicles/pitch--garage-tuning-and-marketplace.md
---

# 0008 — Transact vehicle ownership, upgrades and coins together

## Context

The garage pitch introduces purchases, configurable vehicles and a marketplace
that pays immediately when a player lists a car. Separate balance and ownership
writes would lose cars or mint coins after a failure or concurrent purchase.
The existing profile store already provides asynchronous transactional credits.

## Decision

Extend `ProfileStore` with a garage request/snapshot operation. Each mutation has
a stable transaction ID and catalogue version. SQLite schema 2 migrates schema 1,
adds vehicles, garages and transaction receipts, and grants one starter Corsa
when a profile first opens its garage. The garage row remains after that Corsa is
sold, so reloading cannot grant another. The active vehicle must be owned, and
selling the last or active vehicle is rejected.

`IMMEDIATE` transactions update coins, ownership, configuration and receipts
together. An exact retry returns the current snapshot without repeating the
mutation; reusing an ID with different arguments or another owner fails.
Listings are vehicles owned by the market (`owner IS NULL`), with 20 entries per
page. Listing pays the seller once. A later buyer pays the market, not the seller.

The versioned catalogue defines explicit model names, prices and arcade tuning.
Persisted JSON contains stable model/upgrade identifiers, not ECS entities or enum
ordinals. Validate it on read. Prices use integer arithmetic: floor of 70% base
plus 50% upgrades for buyback; ceiling of 85% base plus 75% upgrades for purchase.
Installed and inactive purchased upgrades remain with the vehicle. Catalogue 1
is fixed; future price changes require a coordinated migration.

The garage gates `GameSystems`, and only confirmed snapshots set `ActiveVehicle`
and `PlayerTuning`. Shop operations are blocked during a drive and while credits
are pending. An uncertain write keeps its request ID and requires retry before
another transaction. Input stays in `input.rs`; the UI emits device-independent
intents. Procedural parts are shared by the road sprites and UI preview.

## Consequences

Ownership and money cannot diverge on a partial commit. JSON adds `serde` and
`serde_json`; model validation remains independent of SQLite and rendering.
The original Corsa handling stays the default for simulation-only tests. Actual
vehicle dimensions now drive road clamping, collisions, pickup clearance and
traffic spacing. Tuning values still require play tests.

Reconstructed alternatives: storing everything as a profile JSON blob would
complicate concurrent ownership transfer; a separate wallet service would require
distributed transactions. One authoritative database keeps the experiment small.

## The tell

Run `cargo test --locked storage::sqlite::garage` and
`cargo test --locked garage_pauses_simulation`. They cover migration, once-only
grants, retries, simultaneous buyers and confirmed selection. `rg -n 'rusqlite'
src` should match only `src/storage/sqlite.rs` and its submodules. Visual checks
must still establish readable models and usable menus.
