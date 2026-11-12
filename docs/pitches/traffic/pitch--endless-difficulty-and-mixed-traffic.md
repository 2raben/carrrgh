# Problem

Progression stops at level 10 (9,000 metres). After that, a long run stays at the
same difficulty. The challenge no longer grows, even though the run can continue
for as long as the player survives. Traffic is also strictly separated by
direction: once a lane holds same-direction or oncoming traffic, every later
vehicle in it shares that direction until the lane clears. Players learn to read
a whole lane as "slow" or "fast", and that makes dense traffic predictable.

# Solution

Remove the cap on levels and make every level at least as hard as the one before.
Add mixed lanes, where oncoming traffic swerves around same-direction traffic, as
a new kind of difficulty that grows with the level.

## Level

- Level `L = floor(metres / 1000) + 1`, with no upper bound. The HUD shows
  `LEVEL L` without "/ 10".
- One pure function maps the level to a difficulty profile, and spawning, lane
  changes and speed read only that profile. Tests and tuning then have one place
  to look.
- Every value in the profile is monotonic in `L`, so a higher level is never
  easier. Values that would break fairness approach a limit instead of passing
  it (see "Fixed limits").

## Two phases

**Levels 1–10** keep today's values, except that mixed lanes are added from
level 3 and evasions from level 6. Each new element arrives on its own level,
as trucks already do.

**From level 11** all parameters move from their level-10 value towards a limit,
using a shared ramp:

```
r(L) = 1 − 0.85^(L − 10)        (L > 10; r(11) ≈ 0.15, r(15) ≈ 0.56,
                                  r(20) ≈ 0.80, r(30) ≈ 0.96)
value(L) = value(10) + (limit − value(10)) · r(L)
```

The level count never ends, and each level is strictly harder than the one
before. In practice, from about level 30 the admission rules, not the curve,
decide how dense traffic can get.

## Difficulty profile

| Parameter | Levels 1–10 | Level 10 | Limit (L → ∞) |
| --- | --- | --- | --- |
| Speed multiplier (cruise and boost) | `1 + 0.05 · (L − 1)` | 1.45 | 1.45 (no further increase) |
| Spawn delay (random range, seconds) | `1.2–2.0 / (1 + 0.15 · (L − 1))` | 0.51–0.85 | 0.25–0.40 |
| Truck share per spawn | 0 % (L1–4), 20 % (L5–7), 35 % (L8–10) | 35 % | 50 % |
| Mixing chance per spawn | 0 % (L1–2), 15 % at L3, +5 points per level | 50 % | 80 % |
| Head-on evasions allowed | from L6 | yes | yes |
| Cooldown after a lane change | 2.0 s | 2.0 s | 1.0 s |
| Retry delay for a blocked lane change | 0.4–1.2 s | 0.4–1.2 s | 0.2–0.6 s |
| Simultaneous voluntary lane changes | 1 | 1 | 2 from L11 (step, not ramped) |

All numbers are play-test starting values, not final balance.

## Fixed limits (never scaled by level)

These rules keep the game fair, so the profile never relaxes them:

- Traffic speeds stay at +140 (same direction) and −150 (oncoming). The player's
  speed stops rising at level 10, so the time to react to an oncoming vehicle does
  not keep shrinking.
- Voluntary lane changes keep their 0.8-second signal and their crossing times.
- The three-lane wall check (`ESCAPE_SPACING`, including the truck allowance), the
  180-unit same-lane spacing and the pickup clearance stay as they are.
- Pickup frequency, coins per metre and hearts do not depend on the level.

## Mixed lanes

Lanes no longer have a fixed direction. A mixing roll lets a spawn ignore the
direction already used in its lane:

- **Diverging mix (from level 3):** a same-direction vehicle may spawn into a lane
  where oncoming vehicles are only *below* it. Relative to the road, the two move
  apart, so no evasion is needed. Only the normal spacing and wall checks apply.
  The same holds for an oncoming vehicle that changes into a lane where
  same-direction traffic is only above it.
- **Head-on mix (from level 6):** an oncoming vehicle may spawn above a
  same-direction vehicle in the same lane. This is only allowed if an **evasion**
  for the oncoming vehicle can be reserved at the same moment.

Right of way: same-direction traffic always keeps its lane. **Oncoming traffic
always gives way** by swerving into an adjacent lane.

## Evasion

An evasion is a lane change of its own kind, faster than a voluntary one:

| | Signal | Crossing | Gap needed at the start* |
| --- | --- | --- | --- |
| Car | 0.5 s | 0.6 s | 290 · 1.1 + 180 ≈ 500 units |
| Truck | 0.5 s | 0.8 s | 290 · 1.3 + 180 ≈ 560 units |

\* The two vehicles close at 140 + 150 = 290 units/s. The evasion has to finish
before their centres come within 180 units of each other.

- The evasion is planned and reserved when the head-on situation is created, not
  later. Its target lane, start time, and both lanes for the whole manoeuvre are
  booked from admission onwards (conservative, like ADR 0006). If no reservation
  passes the spacing, wall and pickup checks, the mix is rejected and the spawn
  falls back to the current same-direction rule.
- The target lane must not create a new head-on conflict for the evading vehicle.
  There are no evasion chains.
- Pairs that cannot come within 180 units before the lower vehicle despawns do not
  count as head-on. They mix without evading.
- Evasions use the same amber signals, driven by simulation time, and the same
  swept collision. The player can see them coming because the head-on pair is
  visible beforehand.
- Evasions are reserved ahead of time. They are not limited by the "one voluntary
  change at a time" rule, but every other vehicle's admission checks still
  respect them.
- Dense traffic in one direction must not lock the other direction out. From
  level 3, four spawns in a row in one direction force the next spawn to use the
  other one.

## Verification at implementation

- Unit tests: the profile is monotonic in `L`, level 1–10 values (apart from
  mixing) match today's, and the limits are never passed.
- Seeded headless simulation at levels 1, 3, 6, 10, 15, 20, 30 and 50, with boost
  cycling. It checks that no traffic overlaps, that no head-on pair lacks a
  reserved evasion, that every evasion keeps 180 units, that no three-lane walls
  form, and that pickup clearance holds.
- In the running game: how readable mixed lanes are, how much warning evasions
  give near the player, and how dense levels 15–30 feel.
- Once built: amend or supersede ADR 0006 (mixed lanes, evasion reservations, two
  simultaneous changes), and update the traffic section of the architecture
  documentation and the HUD.

# Security

All behaviour is local simulation and procedural drawing. There is no new
external content, network access, persistence or dependency. The level is
derived from distance and has no upper bound: use `f64` distance and an integer
level that cannot overflow within any realistic run.

# Rabbit Holes

- **Geometry of evasions:** about 630 units of road are visible ahead of the
  player, and head-on vehicles close at 290 units/s. A normal lane change
  (0.8 + 1.0 s) is too slow for that, so evasions are a separate, faster
  manoeuvre. The decision to avoid a general planner: reserve every evasion at
  admission, or reject the mix.
- **No braking, no speed changes:** traffic keeps its signed speed. An evasion is
  the only way to resolve a head-on conflict.
- **Conservative reservations reduce mixing:** an oncoming vehicle with a planned
  evasion blocks two lanes for some time. Accept this, and refine it only if play
  tests show too little mixing.
- **Saturation:** at high levels, density is limited by the fairness rules, not by
  the curve. Do not relax the wall check or the signal times to get "more".
- **Two simultaneous voluntary changes:** they share the middle lane and need
  longitudinal separation. Use the existing admission checks, not a new
  coordination system.
- **Performance:** a spawn delay of at least 0.25 s keeps the number of vehicles
  small (about 25), so the O(n²) pair check per candidate is enough.

# No-Gos

- Oncoming vehicles do not evade the player. Evading stays the player's job.
- No braking, acceleration or overtaking by traffic, and no traffic-to-traffic
  collisions.
- No speed increase beyond level 10, no shorter signals for voluntary lane
  changes, and no relaxed fairness rules.
- No new vehicle types, lanes or road layouts.
- No difficulty selection, no saved best level, no leaderboard, and no rewards on
  reaching a level.
- Coin rate, pickups and hearts stay independent of the level.
