---
tags: [adr]
status: accepted
date: 2026-11-12
amends: [0006-reserve-lanes-for-signalled-traffic-manoeuvres.md]
sources:
  - src/difficulty.rs
  - src/level.rs
  - src/run.rs
  - src/hud.rs
  - src/tests.rs
  - ../../pitches/traffic/pitch--endless-difficulty-and-mixed-traffic.md
---

# 0010 — Mix lane directions with reserved evasions

## Context

Progression stopped at level 10, and every lane carried a single direction until
it cleared. The
[endless difficulty pitch](../../pitches/traffic/pitch--endless-difficulty-and-mixed-traffic.md)
asks for unbounded levels and for oncoming traffic to give way to same-direction
traffic. Traffic never changes speed, head-on vehicles close at 290 units/s, and
only about 630 units of road are visible ahead of the player. A 1.8-second
voluntary lane change is too slow to resolve a head-on meeting. A general traffic
planner, braking traffic and traffic-to-traffic collisions were rejected as too
large for the gain.

## Decision

Derive one `Difficulty` profile from the unbounded level with a pure function.
Levels 1–10 keep their stepped values. Later levels move monotonically towards
fixed limits by `1 − 0.85^(L − 10)`. Speed, signal times, traffic speeds,
spacing, wall checks and pickup clearance never scale.

Replace "shared lanes require equal signed speeds" with a pairwise lane-sharing
rule. Before level 3 it still requires equal speeds. From level 3 it also admits
diverging pairs that are 180 units apart, and head-on pairs that cannot come
within 180 units before the lower same-direction vehicle leaves at the initial
cruise speed. From level 6 it also admits head-on pairs in which the oncoming
vehicle holds an evasive `LaneChange` that leaves the only shared lane, does not
target the other vehicle's lanes, and releases the lane with 0.05 seconds of
slack before the gap closes. The slack covers fixed-step quantisation of the lane
release.

Evasions are reserved when an oncoming vehicle spawns, never later, and occupy
both lanes from admission, like ADR 0006 reservations. They blink for 0.5 seconds
and cross in 0.6 seconds (trucks 0.8). The existing wall, spacing and pickup
checks consult them, and they do not count towards the voluntary-change limit.
Voluntary changes never create head-on pairs that need an evasion. From level 11,
two voluntary changes may be active, reserved at most one per tick against all
active reservations. From level 3, four consecutive spawns in one direction force
the next spawn to use the other direction.

This amends [ADR 0006](0006-reserve-lanes-for-signalled-traffic-manoeuvres.md).
Its reservation rationale holds. The one-change limit, the equal-speed lane rule
and the ten-level cap are replaced as described above.

## Consequences

Players can no longer read a lane's direction from one vehicle, and difficulty
keeps rising without a level cap. Evasions need a narrow on-screen gap, so they
are rare next to diverging mixed lanes; making them common would need a longer
visible road or faster evasions. Conservative reservations reduce mixing and
density. At high levels the admission rules, not the curve, set traffic density.
The forced direction change can briefly delay spawns in dense traffic. All values
remain play-test tuning.

## The tell

- `cargo test --locked` covers the monotonic profile and its limits, head-on
  admission with and without evasions, evasion timing and lane release, spawned
  evasions only from level 6, and two voluntary changes from level 11.
- A seeded simulation at levels 1–10, 15, 20, 30 and 50 checks lane-sharing
  spacing, overlaps, walls, both directions, mixed lanes and evasions.
- `rg "difficulty_step|/ 10" src` should find nothing. Readability of mixed lanes
  and evasion warnings near the player needs play-testing; tests cannot prove it.
