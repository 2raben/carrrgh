# Architecture

The application implements [Car Drive](../pitches/car-drive/pitch--car-movements.md),
a three-lane arcade driving game replacing the starter platformer, with the
[power-ups and shields](../pitches/power-ups/pitch--power-ups-and-shields.md)
extension. It uses Rust 2024 and Bevy 0.20.1, pinned by [Cargo.toml](../../Cargo.toml)
and [Cargo.lock](../../Cargo.lock). Traffic and pickup randomness uses `fastrand` 2.5.0.
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
| [pickups.rs](../../src/pickups.rs) | Heart and shield pickups: rare random spawning, lane clearance, movement and collection |
| [player.rs](../../src/player.rs) | `Controls`, car-centred `Position`/`PreviousPosition`, movement tuning, lateral steering |
| [run.rs](../../src/run.rs) | Distance, energy, protection, held shield, swept collision, game over and restart |
| [input.rs](../../src/input.rs) | Keyboard and controller intent sampled after `InputSystems` in `PreUpdate` |
| [visuals.rs](../../src/visuals.rs) | Procedural car and pickup sprites, lane marks, interpolation, boost exhaust, shield bubble and protection outline |
| [camera.rs](../../src/camera.rs) | Fixed 960 × 800 road view |
| [hud.rs](../../src/hud.rs) | Hearts, held shield, distance in metres, boost gauge, controls and game-over cue |
| [tests.rs](../../src/tests.rs) | Deterministic tests of the actual simulation |

## Fixed-step flow

At Bevy's default 64 Hz, `GameSystems` chains:

1. Consume the latched restart request. Reset all run state including the held
   shield, the traffic timer, the pickup distance, player current/previous positions
   and controls; reseed randomness and despawn traffic and pickups. Skip driving
   for that tick so the reset state is exact.
2. Unless finished or just restarted, update protection, boost and distance;
   steer the player; attempt an individual traffic spawn when its timer expires;
   attempt a pickup spawn when its distance is due; move traffic and pickups;
   collect pickups; resolve collisions; remove off-screen traffic and pickups.
   Chaining applies deferred spawns before movement and deferred removals before
   the next tick. Collection precedes collisions, so a pickup reached in the same
   tick as a car already counts.

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

## Hearts and shields

Pickups are 36 × 36 boxes lying on the road at a lane centre; they move down at the
player's speed and leave below y = −470. A heart restores one lost heart, up to three.
A shield is held until the next collision, which then still marks the car as hit and
grants protection but costs no heart; it also saves the last heart. At most one
shield is held. A pickup that would have no effect (a heart at full hearts, a second
shield) stays on the road. Collection uses the same swept test as traffic with the
combined half-extents, so boost cannot skip a pickup. Collisions during protection
do not consume the shield.

The next pickup becomes due after a random 400–900 metres of driven distance,
independent of difficulty. At most one pickup is on the road. When due, the kind is
chosen randomly among the useful ones: hearts only while a heart is missing, shields
only while none is held. If neither is useful, the slot is skipped and a new distance
is drawn, so a hit never immediately summons a heart. Pickups spawn at y = 480 in a
shuffled lane; if no lane is clear, the spawn retries each tick.

A lane is clear when every car in it keeps at least 240 units between centres from
the pickup until the pickup passes 78 units below the player's start line. Their gap
changes only by the car's own speed, so checking both ends of the pickup's lifetime
at initial cruise speed (its longest) covers the whole span. Traffic spawning applies
the same check against pickups already on the road. This keeps cars off pickups
in their lane; reaching the lane through other traffic remains the player's risk.
Pickups draw from their own seeded generator, separate from traffic's.

A held shield shows as a translucent cyan bubble around the player and a cyan
shield icon beside the HUD hearts. When a shield absorbs a hit, the protection
outline is white-cyan instead of gold.

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
cycling. Pickup tests cover heart and shield effects and limits, shields with
protection and the last heart, collection at extreme speed, restart, the clearance
prediction, rarity, useful kinds only, one pickup at a time, lane clearance with
real traffic, and seeded reproducibility. Graphical play checks are still required for
readability, input mapping and feel; see the contribution guide.

Keep game behavior in focused ECS systems. New dependencies, persisted formats or
changes to simulation boundaries need an ADR. Update this document with implemented
changes; pitches describe intended work and ADRs preserve earlier rationale.
