---
tags: [adr]
status: accepted
date: 2026-11-15
amends: [0009-verify-drives-in-a-private-market-service.md]
sources:
  - src/bindings.rs
  - src/bindings/persistence.rs
  - src/input.rs
  - src/player.rs
  - src/run.rs
  - src/drive_proof.rs
  - src/market.rs
---

# 0012 — Persist local key bindings and replay braking

## Context

The [key-bindings pitch](../../pitches/key-bindings/pitch--key-bindings.md)
introduces keyboard remapping and a brake. Keyboard preferences belong to the
local device, including when the player uses the private market. Putting them in
profile transactions would couple input preferences to accounts and connectivity.
The replay protocol previously represented steering and boost only, while safety
predictions assumed cruising was the slowest possible forward movement.

## Decision

Keep a separate version-1 JSON preference document with named actions and an
explicit physical-key conversion table. Native settings live beside the profile
save and use I/O-pool jobs, synced temporary files and atomic rename; browser
settings use `car-drive/key-bindings` in localStorage. Validate the complete map
before applying it. Refuse unsupported versions, unknown keys, missing actions,
conflicts and maps outside 1–4 keys per action. On a load error, keep defaults and
show the error; only an edit, restore-defaults or explicit retry replaces the file.

Input capture and menu navigation have exclusive contexts. Capture waits for
release, Escape cancels, and the captured key cannot also activate a menu item.
The simulation still receives only `Controls`; no simulation system reads keys.

Brake wins over boost and recharges energy, but does not clear boost exhaustion
until boost is released. The initial play-test speed is 75% of the tuned cruise
speed, with immediate return to cruise on release. The minimum braking speed also
bounds traffic lifetimes and pickup clearance predictions. It remains above
same-direction traffic speed for every catalogue vehicle.

Record brake alongside steering and boost. Bump the private market protocol to 2
so client and server explicitly agree on this simulation/input change.

## Consequences

Preferences work offline and across local/market profiles without database changes.
The map has a small, explicit supported key vocabulary and uses US physical-key
labels rather than layout-dependent typed characters. G and Escape are reserved;
menu and controller remapping remain outside this pitch.

Invalid settings stay recoverable, with defaults available. Last writer wins if
multiple game instances edit settings; settings are not synchronized across devices.
Client and market server must be upgraded together. More conservative safety
predictions can reject traffic or pickup placements that were valid at cruise.
Braking strength remains subject to play-testing, not a claim of final balance.

## The tell

Run `cargo test --locked bindings`, `cargo test --locked input::tests`,
`cargo test --locked braking`, and `cargo test --locked recorded_drives`.
These check editing, cancellation, storage round trips, precedence and replay parity.
Review `brake_speed` uses in `src/level.rs` and `src/pickups.rs` for prediction drift.
Manually rebind, cancel, reload and restore defaults on native and browser builds;
a passing headless test cannot establish input focus behavior or driving feel.
