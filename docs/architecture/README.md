# Architecture

The application implements [Car Drive](../pitches/car-drive/pitch--car-movements.md),
a three-lane arcade driving game replacing the starter platformer, with the
[power-ups and shields](../pitches/power-ups/pitch--power-ups-and-shields.md)
extension, [lane changes and trucks](../pitches/traffic/pitch--lane-changes-and-trucks.md),
and [coins and a player profile](../pitches/coins/pitch--coins-and-player-profile.md),
plus the [garage, tuning and private marketplace experiment](../pitches/vehicles/pitch--garage-tuning-and-marketplace.md)
and the [browser game](../pitches/browser-game/pitch--browser-game.md).
It uses Rust 2024 and Bevy 0.20.1, pinned by [Cargo.toml](../../Cargo.toml)
and [Cargo.lock](../../Cargo.lock). Traffic and pickup randomness uses `fastrand` 2.5.0.
Native profile storage uses `rusqlite` 0.41 with bundled SQLite and `dirs` 7.0
([ADR 0007](decisions/0007-persist-profiles-behind-a-store-interface.md)); the
browser build keeps one JSON save in `localStorage` through `web-sys`
([ADR 0011](decisions/0011-keep-the-browser-save-as-one-document-in-local-storage.md)).
See [CONTRIBUTING.md](../../CONTRIBUTING.md) for checks and
[ADRs](decisions/README.md) for design history.

## Simulation and presentation

[src/main.rs](../../src/main.rs) composes the simulation, profile, input, visuals,
camera, and HUD plugins. It gives the profile plugin the SQLite or network store
natively and the `localStorage` store in the browser, and chooses the window: 960 × 800
natively, the page's `#car-drive` canvas in the browser. Simulation runs headless under `MinimalPlugins`, following
[ADR 0002](decisions/0002-keep-the-simulation-headless-behind-a-controls-resource.md).
Only input reads devices; only presentation writes `Transform`.

| Module | Responsibility |
| --- | --- |
| [simulation.rs](../../src/simulation.rs) | Resources, player startup, fixed-step ordering |
| [difficulty.rs](../../src/difficulty.rs) | Pure mapping from the endless level to speed and traffic tuning |
| [level.rs](../../src/level.rs) | Road bounds, lane centres, traffic components, vehicle kinds, lane reservations, mixed lanes, random spawning, signalled lane changes and evasions, movement and cleanup |
| [pickups.rs](../../src/pickups.rs) | Heart and shield pickups: rare random spawning, lane clearance, movement and collection |
| [player.rs](../../src/player.rs) | `Controls`, car-centred `Position`/`PreviousPosition`, movement tuning, lateral steering |
| [run.rs](../../src/run.rs) | Distance, coins, energy, protection, held shield, swept collision, game over, restart and the `RunEnded` message |
| [profile.rs](../../src/profile.rs) | `ProfilePhase` state, loading or creating the profile, queued coin credits, background storage jobs and the exit flush |
| [storage.rs](../../src/storage.rs) | `ProfileStore` contract and its data types; [sqlite.rs](../../src/storage/sqlite.rs) for native builds; [document.rs](../../src/storage/document.rs) with [browser.rs](../../src/storage/browser.rs) for the browser and [memory.rs](../../src/storage/memory.rs) for tests; [contract.rs](../../src/storage/contract.rs) shared garage scenarios |
| [input.rs](../../src/input.rs) | Keyboard and controller intent sampled after `InputSystems` in `PreUpdate`; name typing while creating a player |
| [visuals.rs](../../src/visuals.rs) | Procedural car, truck and pickup sprites, turn signals, lane marks, interpolation, boost exhaust, shield bubble and protection outline |
| [camera.rs](../../src/camera.rs) | Fixed 960 × 800 road view |
| [hud.rs](../../src/hud.rs) | Player name, coin balance and drive coins, storage problems, hearts, held shield, distance in metres, difficulty level, boost gauge, controls, game-over cue and create-player screen |
| [vehicles.rs](../../src/vehicles.rs) | Stable catalogue/configuration types, prices, upgrades and vehicle-specific simulation tuning |
| [garage.rs](../../src/garage.rs), [garage_ui.rs](../../src/garage_ui.rs) | Garage gating, confirmed asynchronous transactions, keyboard/controller/pointer menus and purchase previews |
| [vehicle_art.rs](../../src/vehicle_art.rs) | Shared procedural model, paint, tire and body-kit shapes for garage and road |
| [market.rs](../../src/market.rs), [drive_proof.rs](../../src/drive_proof.rs) | Private authenticated service/client, bounded input recording and headless authoritative replay (service, client and replay native only) |
| [tests.rs](../../src/tests.rs) | Deterministic tests of the actual simulation |

## Fixed-step flow

At Bevy's default 64 Hz, `GameSystems` chains:

1. If a restart is latched, report the current drive as ended (unless it was
   already reported at game over). Then consume the latched restart request. Reset all run state including the held
   shield, the traffic timer, the pickup distance, player current/previous positions
   and controls; reseed randomness and despawn traffic and pickups. Skip driving
   for that tick so the reset state is exact.
2. Unless finished or just restarted, update protection, boost and distance;
   steer the player; attempt an individual traffic spawn when its timer expires;
   attempt a pickup spawn when its distance is due; reserve a safe adjacent lane
   for a signalled change; move traffic and pickups;
   collect pickups; resolve collisions; remove off-screen traffic and pickups.
   Chaining applies deferred spawns before movement and deferred removals before
   the next tick. Collection precedes collisions, so a pickup reached in the same
   tick as a car already counts.
3. If the drive is over, report it as ended, once.

The profile plugin runs `GameSystems` only in `ProfilePhase::Playing`. The garage
plugin additionally gates it while the garage is open. After a restart, online
ticket seeds prepare the traffic/pickup generators, then each driving tick records
quantized input before advancing the simulation. Without these plugins, as in
most headless tests, the drive starts immediately.

Held steering and boost apply each tick; restart is consumed once. Game over gates
simulation itself, not just input. In `PostUpdate`, positions and road distance
interpolate between the last two ticks before transform propagation. At game over,
rendering uses the final positions directly, avoiding interpolation cycling while
simulation is stopped.

## Road and movement

The road spans x = −210…210 with lane centres −140, 0, 140. The default Corsa and
ordinary traffic cars have a 44 × 78 collision box anchored at their centre;
trucks use 70 × 144. Player dimensions come from the selected model's tuning,
from a 30 × 42 Bobby-Car to a 76 × 110 Fendt. Player road clamping, swept traffic
and pickup collision tests use these dimensions.
The player starts at (140, −230), moves
sideways at up to 420 units/s, stops on release, and is clamped inside the road.
The camera keeps the same world extent on resize, preserving reaction distance;
unusual aspect ratios stretch the view.

Cruising initially advances at 300 units/s; boost reaches 540. Road marks scroll by
travelled distance, while traffic moves at its own signed speed minus player speed. Positive
traffic speed (140) is same-direction; negative (−150) is oncoming. Front windows,
headlights and opposite orientation distinguish direction as well as colour.
Ten world units count as one displayed metre. Distance uses `f64` to keep long
runs and scrolling stable.

Levels are endless: level `L = floor(metres / 1000) + 1`, shown in the HUD as
`LEVEL L`. `Difficulty::at` in `difficulty.rs` maps the level to one profile that
speed, spawning and lane changes read. Every value is monotonic in the level.
Levels 1–10 step: cruise and boost gain 5% of their initial values per level up to
435 and 783 units/s at level 10, and the spawn delay shrinks. From 4,000 metres
(level 5), each spawn has a 20% chance of a truck, increasing to 35% at 7,000
metres. Mixed lanes start at level 3 with a 15% mixing chance, plus 5 points per
level; head-on evasions start at level 6. From level 11, speed stays fixed, and the
other values move from their level-10 value towards a limit by
`r(L) = 1 − 0.85^(L − 10)`: spawn delay 0.51–0.85 s → 0.25–0.40 s, trucks
35% → 50%, mixing 50% → 80%, lane-change cooldown 2.0 → 1.0 s, blocked-change
retry 0.4–1.2 → 0.2–0.6 s. Two voluntary lane changes may run at once from level 11.
Signal times, traffic speeds, spacing, wall and pickup checks never scale, so at
high levels the admission rules rather than the curve limit density.
Speed changes on the tick after crossing a threshold; existing traffic retains its
signed speed. Restart resets progression through zero distance and starts a fresh
random traffic sequence. No trucks appear before 4,000 metres.
Trucks have a cargo bed and cab, the larger collision box, and slower lane changes.

Cars spawn individually at y = 480, even while previous cars remain on the road,
and leave below y = −470. Each spawn shuffles the candidate lanes. Cars get a random
lateral offset up to 36 units from the lane centre; trucks up to 25 units, keeping
their body in the lane. Lane identity is stored explicitly rather than inferred from
the offset x position. An empty lane randomly receives same-direction or oncoming
traffic; subsequent cars in an occupied or reserved lane take the direction of its
nearest (highest) vehicle unless the spawn mixes, keeping at least 180 units between
the centres of vehicles sharing a lane. The next random delay is drawn from the
level's range (1.2–2.0 seconds at level 1). Thus later stages create denser
traffic without simultaneous batches. If no lane is safe, the spawner retries after
0.1 seconds.

Before admission, each candidate is checked against pairs of existing cars in the
other lanes. Intersecting their relative-motion intervals detects whether all three
could fit within a 360-unit longitudinal band (426 with trucks) before any of them
despawns. Lane reservations are included in every possible assignment to three
lanes. Such candidates are rejected. The lifetime estimate uses initial cruise speed, so boost,
boost release and increasing difficulty cannot extend the predicted conflict window.
This prevents compact three-lane walls and same-lane overlaps; it does not establish
that every possible steering choice can recover safely. Headless tests use explicit
random seeds; normal play and restart use fresh seeds. See
[ADR 0005](decisions/0005-spawn-continuous-random-traffic.md) for the change from waves.

Vehicles can change into an adjacent lane while between y = 160 and 360. They
blink amber at both ends of the intended side for 0.8 seconds before lateral motion,
then continue blinking through a smooth 1.0-second crossing (1.4 seconds for trucks).
Blinking is 2 Hz and uses simulation time; oncoming cars reverse their local light
side to indicate the correct road direction. Signals freeze on game over and stop
when the vehicle reaches the new random offset. Position interpolation also covers
lateral movement, and swept collisions include the crossing trajectory.

Both source and destination lanes are reserved from the first warning until the
change finishes. One voluntary change is active at a time (two from level 11); the
planner reserves at most one per tick, checked against every active reservation.
The planner uses the same lane-sharing, spacing and future-wall checks as spawning,
and refuses to close a pickup's clearance in either reserved lane. Cars never
accelerate or reverse to merge. New traffic and pickups respect reservations.
Blocked changes retry after the level's random retry delay while still in the
planning zone; a finished change waits the level's cooldown. See
[ADR 0006](decisions/0006-reserve-lanes-for-signalled-traffic-manoeuvres.md).

Before level 3, vehicles sharing a lane must have the same signed speed. From
level 3, a spawn rolls the level's mixing chance once when an occupied lane is
tried. A mix first tries the opposite direction to the lane's nearest vehicle and
falls back to the lane's direction. Mixed pairs sharing a lane are admitted when:

- they diverge (same-direction vehicle ahead) and are already 180 units apart;
- they approach head-on, but the lower same-direction vehicle leaves the road
  (estimated at initial cruise speed) before they come within 180 units; or
- the oncoming vehicle already holds an **evasion** out of the only shared lane
  that releases it, plus 0.05 seconds of fixed-step slack, before the gap closes
  below 180 units. Its target lane must not hold the same-direction vehicle.

Oncoming traffic always gives way. From level 6, an oncoming spawn that is only
blocked by a head-on pair may be admitted with an evasion into a random adjacent
lane, reserved from admission. Evasions blink for 0.5 seconds and cross in 0.6
seconds (0.8 for trucks), with the usual signals, interpolation and swept
collisions. They do not count towards the voluntary limit, and voluntary changes
never create head-on pairs that need one. Because head-on traffic closes at 290
units/s and the gap must fit an evasion, evasions occur only when the
same-direction vehicle is in a narrow band in front of the player; they are rare
compared with diverging mixed lanes. From level 3, four spawns in a row in one
direction force the next spawn to use the other direction, waiting for a lane
that admits it. Otherwise dense same-direction traffic can lock oncoming traffic
out. See [ADR 0010](decisions/0010-mix-lane-directions-with-reserved-evasions.md).

## Boost and damage

The tank starts full, drains at 40%/s and recharges at 20%/s without boost. The last
partially powered tick receives a proportional speed increase. Emptying the tank
requires releasing boost before it can activate again; it can recharge while the
button remains held. The HUD shows remaining energy and the car has exhaust while
boosting.

A swept relative segment against the box expanded by the sum of the player's and
the traffic vehicle's half-extents uses Bevy's `RayCast2d` to catch collisions between
tick endpoints, including simultaneous lateral motion.
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

A lane is clear when every car occupying or reserving it keeps at least 240 units
between centres from the pickup until it passes 78 units below the player's start line. Their gap
changes only by the car's own speed, so checking both ends of the pickup's lifetime
at initial cruise speed (its longest) covers the whole span. Traffic spawning applies
the same check against pickups already on the road. This keeps cars off pickups
in their lane; reaching the lane through other traffic remains the player's risk.
Pickups draw from their own seeded generator, separate from traffic's.

A held shield shows as a translucent cyan bubble around the player and a cyan
shield icon beside the HUD hearts. When a shield absorbs a hit, the protection
outline is white-cyan instead of gold.

Garage models amend these baseline values: the traffic wall spacing grows for
larger player dimensions and slower steering, and pickup clearance/lifetime
grows for a longer player. Minimum cruising speed remains above same-direction
traffic speed; the fastest tuned model stays below 1,000 units/s from level ten on.
The conservative checks do not prove that every player steering choice can recover.

These choices supersede platformer movement in
[ADR 0004](decisions/0004-drive-in-road-relative-coordinates.md). Tuning values are
initial play-test values, not claims of final balance.

## Profiles and coins

A drive earns one coin per 10 metres, rounded down; boost, difficulty and hits do
not change the rate. `Run` reports the drive's end once through a `RunEnded`
message: at game over, when restarting mid-drive, or, from the profile plugin,
when the app exits during a drive. The HUD shows the player's name, the confirmed
balance and the coins of the current drive.

`ProfilePhase` starts in `Loading`. The stored profile, if any, moves it to
`Playing`. Without one it moves to `Creating`: input stops sampling `Controls`
and instead edits a `NameDraft` (prefilled with "Driver"; text, Backspace,
Enter, or controller South/Start to confirm). Names are trimmed, 1–16 characters
and without control characters. The drive starts once the store confirms the new
profile; a failed save is shown and can be retried. If loading fails (unreadable,
foreign or newer file), the game plays without saving and the HUD says so.

Each `RunEnded` with coins queues a `RunCredit` with a random run id for the
current profile. One storage operation runs at a time on `IoTaskPool`; results
apply in `Update`, so the credits are confirmed in order. A failed credit stays at
the front of the queue, the problem is shown, and it is retried at the next drive's
end or on exit. The game-over cue shows the new balance only after confirmation.
On `AppExit`, the `Last` schedule waits for the running operation and writes the
outstanding credits synchronously.

`SqliteStore` keeps `profiles.sqlite3` in `$CAR_DRIVE_DATA_DIR` or in
`car-drive` under the platform data directory (for example
`~/.local/share/car-drive` on Linux). It opens lazily, initialises an empty file
to schema 2, migrates schema 1, and refuses foreign or newer files without writing
to them. A credit inserts its
run id and updates the balance in one transaction; a repeated run id changes
nothing. Stored data is validated again on load. See
[ADR 0007](decisions/0007-persist-profiles-behind-a-store-interface.md).

In the browser, `BrowserStore` keeps the whole save as one `SaveDocument` in the
`localStorage` item `car-drive/save` (format `car-drive-save`, version 1). Each
operation reads and decodes the item, applies the change to a copy and writes it
back with one `setItem` only if something changed, before the operation's future
is returned. The document holds profiles, vehicles with their owner or none for
a listing, garages, and the latest 256 credited runs and garage receipts. Its
garage operations mirror the SQLite transaction. An invalid item moves to
`car-drive/save.broken`; a newer one is refused and left untouched. `MemoryStore`
keeps the same encoded document for tests. Closing the tab sends no `AppExit`, so
only a drive in progress is lost. See
[ADR 0011](decisions/0011-keep-the-browser-save-as-one-document-in-local-storage.md).

## Garage and private shared marketplace

After the profile loads, the garage opens before the first drive. A schema-2
garage row records the once-only starter Corsa and active vehicle. Model-specific
parameters, color, tires, two motor stages and a carbon kit persist per vehicle.
Bought equipment can be switched off and back on without paying again. Integer
catalogue prices and upgrade rules live in `vehicles.rs` and remain fixed at
catalogue version 1. Repeated equipment changes recompute from base tuning.

`ProfileStore::garage` exchanges versioned requests and confirmed snapshots.
SQLite implementation lives under `storage/sqlite/`. One `IMMEDIATE` transaction
debits/credits coins, transfers ownership and records the request ID. An exact
retry does not mutate twice. The marketplace guarantees immediate buyback, then
holds that vehicle until someone buys it; there is no second seller payout.
The garage cannot sell the last or active vehicle. Market listings are paged in
groups of twenty. Pending writes and credits block further purchases and drive
starts; uncertain transport results retain their original request ID for retry.
See [ADR 0008](decisions/0008-transact-vehicle-ownership-and-tuning.md).

`--market-server` runs a separate private service; `--market-provision` creates
operator-approved credentials. `CAR_DRIVE_MARKET_CONFIG` selects a network store
instead of a local save. Each database has a local/market realm marker to prevent
cross-use. The service authenticates tokens, binds only loopback and requires an
SSH tunnel for another machine. No public service is deployed. It serializes
authenticated operations and uses the same atomic garage implementation.

Online drives begin with a server-issued seed and vehicle snapshot. At 64 Hz the
client records quantized steering and boost, then submits those inputs on game
over, R/Select, or exit. The server replays the real `SimulationPlugin` to calculate
coins, checks elapsed server time and credits each ticket exactly once. No client
score or local balance is accepted. Garage mutations are locked until the ticket
finishes. A new client login abandons an older unfinished ticket. Online drives
end after twenty minutes to bound memory and verification time. Local drives
keep their original restart behavior and have no recording limit. See
[ADR 0009](decisions/0009-verify-drives-in-a-private-market-service.md) and the
[operator guide](../marketplace.md).

Additional tests cover all catalogue models and tuning, replay parity, actual
collision dimensions, garage pause/confirm/start flow, schema migration, realm
separation, once-only grants, pricing, retries, concurrent buyers and authenticated
TCP ownership transfers. A physical-controller and two-machine SSH test remain
outside automated coverage.

## Browser build and publishing

The same crate builds for `wasm32-unknown-unknown` with Bevy's `web` feature and
WebGL2. [web/index.html](../../web/index.html) shows the 6:5 field, "Loading …"
and a hint without WebGL2, then focuses the canvas. Bevy suppresses the browser's
default key handling on the canvas. Escape does not quit in the browser
(`input::CAN_QUIT`), and the HUD leaves it out of the controls. `fastrand` seeds
from the browser's crypto through its `js` feature. The private market, its
client, SQLite and drive verification compile natively only, so the browser
plays with the local market.

`mise run web:build` builds the size-optimized `web` profile, runs `wasm-bindgen`
(pinned to the `Cargo.lock` version) and `wasm-opt`, and writes `dist/web`.
`mise run web:serve` serves it locally. CI runs `mise run check` and Clippy for
`wasm32` on pull requests; the Pages workflow publishes `dist/web` on every push to
`main`.

## Validation and conventions

`mise run check` runs formatting, Clippy, build, and tests in the project Nix shell
on NixOS. Headless tests cover steering and edges, boost depletion/recharge,
protection and repeated contact, tunnelling and near misses, relative traffic
motion and cleanup, game-over freeze, complete restart, difficulty thresholds,
the monotonic endless profile and its limits, truck unlock and dimensions, lateral
offsets, warning-before-motion and completion, safe lane reservations, two
simultaneous changes from level 11, head-on admission with and without evasions,
evasion timing and lane release, spawned evasions only from level 6, damage while
waiting on lane markings, reproducible random layouts, continuous individual
spawning, and lane-sharing spacing, mixed lanes and three-lane wall checks across
seeds at levels 1–10, 15, 20, 30 and 50 with boost cycling. Pickup tests cover heart and shield effects and limits, shields with
protection and the last heart, collection at extreme speed, restart, the clearance
prediction, rarity, useful kinds only, one pickup at a time, lane clearance with
real traffic, and seeded reproducibility. Profile tests cover the coin rate,
starting with a stored profile, no drive before a valid name is saved, exactly-once
credits across game over, restart and exit, retrying failed credits, and playing on
when storage is unreadable. SQLite tests cover reloading, idempotent and atomic
credits, initialising empty files, refusing foreign, newer and non-database files
unchanged, rejecting edited invalid names, and one end-to-end run through the
plugin and the I/O pool. The contract tests run the same garage and credit
scenarios against SQLite and the save document; document tests cover reopening,
edited, foreign, newer and oversized saves, no write after a failure, and bounded
receipts. The browser's `localStorage` glue is checked manually. Graphical play checks are still required for
readability, input mapping and feel; see the contribution guide.

Keep game behavior in focused ECS systems. New dependencies, persisted formats or
changes to simulation boundaries need an ADR. Update this document with implemented
changes; pitches describe intended work and ADRs preserve earlier rationale.
