---
tags: [adr]
status: accepted
date: 2026-11-12
amends: [0005-spawn-continuous-random-traffic.md]
amended-by: [0010-mix-lane-directions-with-reserved-evasions.md]
sources:
  - src/level.rs
  - src/pickups.rs
  - src/run.rs
  - src/visuals.rs
  - src/tests.rs
---

# 0006 — Reserve lanes for signalled traffic manoeuvres

> Amended by [ADR 0010](0010-mix-lane-directions-with-reserved-evasions.md): levels
> are no longer capped at ten, lanes may mix directions from level 3, oncoming
> traffic may hold reserved evasions from level 6, and two voluntary changes may
> run at once from level 11. The reservation rationale below still holds.

## Context

Exact lane-centre traffic leaves lane markings permanently safe. Lateral offsets,
lane changes and larger trucks invalidate lane identity inferred from x equality.
The existing predictive spacing and pickup clearance must survive those changes.

## Decision

Store lane identity and vehicle kind on `Traffic`, separately from `Position`.
Store a manoeuvre's start, target lane/offset and elapsed simulation time there too.
Reserve source and destination lanes for its full duration, including the 0.8-second
stationary warning. Admit at most one active manoeuvre at a time, to keep reservation
order explicit without a general traffic planner. Cars cross in 1.0 seconds,
trucks in 1.4 seconds, using smoothstep interpolation in the fixed simulation.

Use the same admission check for spawns and manoeuvres: shared lanes require equal
signed speeds and 180 units between centres; every pair of other vehicles must
pass the predictive three-lane wall check. Test all assignments of reserved lanes,
and increase the 360-unit wall band by the largest extra vehicle length. Pickup
clearance consults lane reservations in both spawning directions and before changes.

Use each vehicle kind's size for drawing and swept collision half-extents. Turn
signals read manoeuvre time rather than a render clock, so they freeze with the run.
Signals are attached to both ends of the selected side, reversing local side for
oncoming vehicles. No parallel simulation loop or physics dependency is introduced.

This amends [ADR 0005](0005-spawn-continuous-random-traffic.md). Its timer,
randomness and spacing rationale remain in force; lane-centre identity is replaced
by explicit reservations. Difficulty now advances every 1,000 metres through ten
levels, with trucks eligible from 4,000 metres.

## Consequences

Lane markings no longer offer permanent immunity. Conservative reservations may
defer lane changes and spawns, especially with opposing traffic or pickups. Only
adjacent changes are allowed; vehicles retain their signed speed during a change.
The bounds preserve headless testing and a small live traffic population. They
exclude compact walls but do not guarantee recovery from every player decision.
Warning timing and later-level density remain play-test values.

## The tell

- `cargo test --locked` checks warning-before-motion, both change directions,
  larger collision extents, the truck threshold, and stationary lane-marking damage.
- Seeded simulation checks all ten difficulty levels with boost cycling for
  reservation spacing, traffic overlap, future three-lane walls and pickup clearance.
- In the running game, inspect both directions' indicators, trucks, HUD levels,
  game-over freeze and restart. Automated geometry tests do not prove visual clarity
  or comfortable reaction time.
