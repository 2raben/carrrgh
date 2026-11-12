---
tags: [adr]
status: accepted
date: 2026-11-12
supersedes: [0001-move-the-player-with-hand-written-kinematics.md]
amends: [0002-keep-the-simulation-headless-behind-a-controls-resource.md]
amended-by: [0005-spawn-continuous-random-traffic.md]
sources:
  - src/player.rs
  - src/level.rs
  - src/run.rs
  - src/simulation.rs
  - src/tests.rs
  - ../../pitches/car-drive/pitch--car-movements.md
---

# 0004 — Drive in road-relative coordinates with swept collisions

Traffic generation is amended by
[ADR 0005](0005-spawn-continuous-random-traffic.md): continuous random individual
spawns replace the waves described below. The original rationale is preserved.

## Context

[Car Drive](../../pitches/car-drive/pitch--car-movements.md) replaces the platformer.
It needs continuous lateral steering, same-direction and oncoming traffic, and
boost. Grounding, gravity and tile collision no longer describe gameplay. Fast
relative movement must not skip a collision between simulation ticks.

Alternatives considered: an ever-growing world coordinate and following camera
would work but complicate spawning and long-run precision. Rigid-body physics
would add collision response and rotation that this pitch does not need.

## Decision

Retain Bevy's 64 Hz fixed simulation and headless `Controls` boundary. Keep the
player at a fixed longitudinal coordinate; simulate traffic velocity relative to
player speed. Accumulate distance separately for the score and road scrolling.
Use centre-anchored boxes and Bevy's ray/AABB intersection on the relative movement
segment, expanding the target by the player's half-size. Resolve damage once per
car with a global protection interval. Cars do not physically push one another.

Use six repeatable waves with one unoccupied lane. Spawn a new wave only after
all previous traffic exits and a cooldown completes. This bounds entity count and
prevents fast opposing traffic closing gaps in slower waves.

[ADR 0001](0001-move-the-player-with-hand-written-kinematics.md) is superseded; its
platformer rationale remains historical. This record amends the concrete components
and controls of [ADR 0002](0002-keep-the-simulation-headless-behind-a-controls-resource.md):
traffic replaces coins, `boost` replaces jump, and restart is the only latched edge.
The separation between simulation and presentation remains accepted.

## Consequences

- No new dependencies or second game loop. Tests use the actual simulation.
- Swept collision handles high relative speeds and simultaneous lateral movement.
- Separated waves are deliberately simple and have quiet gaps, especially when
  catching same-direction cars. Complex traffic AI is outside this prototype.
- Camera extents stay fixed; unusual window aspect ratios stretch the view.
- Tuning still requires play-testing. Deterministic evasion tests establish an
  available path, not universal difficulty or accessibility claims.

## The tell

- `cargo test --locked` covers tunnelling, lateral near misses, each wave at full
  boost, immunity, frozen game over and complete restart.
- `rg 'Transform|Sprite|KeyCode|Gamepad' src/player.rs src/level.rs src/run.rs src/simulation.rs`
  finds no simulation dependence on presentation or devices.
- Manual play confirms the road scrolls, traffic fronts read correctly, boost
  changes relative speed, and the protection outline and HUD match state.
