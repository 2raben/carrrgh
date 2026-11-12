# Architecture

The application implements [Car Drive](../pitches/car-drive/pitch--car-movements.md),
a three-lane arcade driving game replacing the starter platformer. It uses Rust
2024 and Bevy 0.20.1, pinned by [Cargo.toml](../../Cargo.toml) and
[Cargo.lock](../../Cargo.lock). Traffic randomness uses `fastrand` 2.5.0.
See [CONTRIBUTING.md](../../CONTRIBUTING.md) for checks and
[ADRs](decisions/README.md) for design history.

## Simulation and presentation

[src/main.rs](../../src/main.rs) composes the simulation, input, visuals, camera,
and HUD plugins. Simulation runs headless under `MinimalPlugins`, following
[ADR 0002](decisions/0002-keep-the-simulation-headless-behind-a-controls-resource.md).
Only input reads devices; only presentation writes `Transform`.

| Module | Responsibility |
| --- | --- |
| [simulation.rs](../../src/simulation.rs) | Resources, player startup, fixed-step ordering |
| [level.rs](../../src/level.rs) | Road bounds, lane centres, traffic components, random spawning with spacing checks, movement and cleanup |
| [player.rs](../../src/player.rs) | `Controls`, car-centred `Position`/`PreviousPosition`, movement tuning, lateral steering |
| [run.rs](../../src/run.rs) | Distance, energy, protection, swept collision, game over and restart |
| [input.rs](../../src/input.rs) | Keyboard and controller intent sampled after `InputSystems` in `PreUpdate` |
| [visuals.rs](../../src/visuals.rs) | Procedural car sprites, lane marks, interpolation, boost exhaust and protection outline |
| [camera.rs](../../src/camera.rs) | Fixed 960 × 800 road view |
| [hud.rs](../../src/hud.rs) | Hearts, distance in metres, boost gauge, controls and game-over cue |
| [tests.rs](../../src/tests.rs) | Deterministic tests of the actual simulation |

## Fixed-step flow

At Bevy's default 64 Hz, `GameSystems` chains:

1. Consume the latched restart request. Reset all run state, the traffic timer,
   player current/previous positions and controls; reseed randomness and despawn traffic. Skip driving
   for that tick so the reset state is exact.
2. Unless finished or just restarted, update protection, boost and distance;
   steer the player; attempt an individual traffic spawn when its timer expires;
   move traffic; resolve collisions; remove off-screen traffic. Chaining applies deferred spawns
   before movement and deferred removals before the next tick.

Held steering and boost apply each tick; restart is consumed once. Game over gates
simulation itself, not just input. In `PostUpdate`, positions and road distance
interpolate between the last two ticks before transform propagation. At game over,
rendering uses the final positions directly, avoiding interpolation cycling while
simulation is stopped.

## Road and movement

The road spans x = −210…210 with lane centres −140, 0, 140. Cars have a 44 × 78
collision box anchored at their centre. The player starts at (140, −230), moves
sideways at up to 420 units/s, stops on release, and is clamped inside the road.
The camera keeps the same world extent on resize, preserving reaction distance;
unusual aspect ratios stretch the view.

Cruising initially advances at 300 units/s; boost reaches 540. Road marks scroll by
travelled distance, while traffic moves at its own signed speed minus player speed. Positive
traffic speed (140) is same-direction; negative (−150) is oncoming. Front windows,
headlights and opposite orientation distinguish direction as well as colour.
Ten world units count as one displayed metre. Distance uses `f64` to keep long
runs and scrolling stable.

Every 1,500 metres, cruise and boost speed gain 5% of their initial values and
traffic spawn frequency increases. Progression caps after six steps (9,000 metres),
at 390 units/s cruising and 702 boosting. Speed changes on the tick after crossing
a threshold; existing traffic retains its signed speed. Restart resets progression
through zero distance and starts a fresh random traffic sequence.

Cars spawn individually at y = 480, even while previous cars remain on the road,
and leave below y = −470. Each spawn shuffles the candidate lanes. An empty lane
randomly receives same-direction or oncoming traffic; subsequent cars in that lane
share its speed until it clears, keeping at least 180 units between their centres.
The next random delay is 1.2–2.0 seconds divided by `1 + 0.15 × difficulty_step`.
Thus later stages create denser traffic without simultaneous batches. If no lane
is safe, the spawner retries after 0.1 seconds.

Before admission, each candidate is checked against pairs of existing cars in the
other lanes. Intersecting their relative-motion intervals detects whether all three
could fit within a 360-unit longitudinal band before any of them despawns. Such
candidates are rejected. The lifetime estimate uses initial cruise speed, so boost,
boost release and increasing difficulty cannot extend the predicted conflict window.
This prevents compact three-lane walls and same-lane overlaps; it does not establish
that every possible steering choice can recover safely. Headless tests use explicit
random seeds; normal play and restart use fresh seeds. See
[ADR 0005](decisions/0005-spawn-continuous-random-traffic.md) for the change from waves.

## Boost and damage

The tank starts full, drains at 40%/s and recharges at 20%/s without boost. The last
partially powered tick receives a proportional speed increase. Emptying the tank
requires releasing boost before it can activate again; it can recharge while the
button remains held. The HUD shows remaining energy and the car has exhaust while
boosting.

A swept relative segment against the expanded car box uses Bevy's `RayCast2d` to
catch collisions between tick endpoints, including simultaneous lateral motion.
A hit removes one of three hearts and grants 1.4 seconds of protection, shown by a
steady gold outline. Each traffic car can damage the player only once. No collision
impulses or rotation physics are simulated. The third hit freezes the drive and
shows distance and restart controls.

These choices supersede platformer movement in
[ADR 0004](decisions/0004-drive-in-road-relative-coordinates.md). Tuning values are
initial play-test values, not claims of final balance.

## Validation and conventions

`mise run check` runs formatting, Clippy, build, and tests in the project Nix shell
on NixOS. Headless tests cover steering and edges, boost depletion/recharge,
protection and repeated contact, tunnelling and near misses, relative traffic
motion and cleanup, game-over freeze, complete restart, difficulty thresholds and
caps, reproducible random layouts, continuous individual spawning, and same-lane
spacing and three-lane wall checks across seeds and difficulty levels with boost
cycling. Graphical play checks are still required for
readability, input mapping and feel; see the contribution guide.

Keep game behavior in focused ECS systems. New dependencies, persisted formats or
changes to simulation boundaries need an ADR. Update this document with implemented
changes; pitches describe intended work and ADRs preserve earlier rationale.
