---
tags: [adr]
status: accepted
date: 2026-11-12
amends: [0004-drive-in-road-relative-coordinates.md]
amended-by: [0006-reserve-lanes-for-signalled-traffic-manoeuvres.md]
sources:
  - src/level.rs
  - src/run.rs
  - src/tests.rs
  - Cargo.toml
---

# 0005 — Spawn continuous random traffic with predictive spacing checks

[ADR 0006](0006-reserve-lanes-for-signalled-traffic-manoeuvres.md) amends lane
identity, reservations and vehicle sizes, and updates the difficulty thresholds.

## Context

The player requested random arrangements instead of repeating traffic waves.
Waiting for all cars to leave creates artificial pauses; unconstrained independent
spawning can let fast oncoming cars close gaps around slower traffic. The existing
1,500-metre difficulty progression must continue to increase speed and density.

## Decision

Replace the wave sequence with a fixed-step timer that spawns at most one car at
a time, without waiting for previous traffic to clear. Shuffle candidate lanes,
randomize direction when a lane is empty, and randomize the next delay. Increase
spawn frequency by dividing that delay by `1 + 0.15 × difficulty_step`.

Keep same-lane cars at the same signed speed with a minimum 180-unit centre gap.
For every candidate and pair of cars in the other lanes, analytically intersect
the time intervals when all three pairs are within 360 longitudinal units. Reject
the candidate if this can occur before any car exits. Estimate exit time at initial
cruise speed, which conservatively covers subsequent boosts and difficulty changes.
Retry after 0.1 seconds when no candidate is safe; do not accumulate spawn batches.

Declare `fastrand` 2.5.0 directly, using the version already in Cargo.lock. Store
its RNG in `TrafficFlow`; normal runs and restarts receive fresh seeds, while tests
inject seeds for reproducible cases. This avoids maintaining a custom generator.

This amends only traffic generation in
[ADR 0004](0004-drive-in-road-relative-coordinates.md). Road-relative coordinates,
swept collision, fixed scheduling and the headless boundary remain unchanged.

## Consequences

Cars appear continuously in varied arrangements, with denser attempts at later
stages. Spacing checks may delay a spawn, so frequency is a target rather than an
exact throughput guarantee. Same-lane direction is coherent until that lane clears.
Compact three-lane walls are excluded; arbitrary player choices are not guaranteed
recoverable, and these tuning distances still need manual play-testing.

The pair checks are quadratic in live traffic. The minimum spacing and finite
spawn-to-despawn road length bound the population to a small number of cars.
The game adds a direct dependency but no new package version to the lockfile.

## The tell

- `rg 'WAVES|flow.wave' src` should find no wave sequence or wave index.
- `cargo test --locked` checks individual overlapping arrivals, variable intervals,
  seeded reproducibility, distance progression, and spacing over multiple seeds and
  all difficulty levels while boost cycles on and off. A focused case checks an
  oncoming car closing a future gap rather than only inspecting current positions.
- Play through cruise, boost, release and restart. Check varied lane occupancy,
  readable traffic direction and usable gaps; finite seeded tests do not prove
  universal playability or comfortable reaction time.
