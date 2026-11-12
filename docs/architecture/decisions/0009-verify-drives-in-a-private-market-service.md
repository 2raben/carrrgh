---
tags: [adr]
status: accepted
date: 2026-11-12
amends: [0007-persist-profiles-behind-a-store-interface.md]
sources:
  - src/market.rs
  - src/drive_proof.rs
  - src/storage/sqlite/market.rs
  - src/profile.rs
  - src/simulation.rs
---

# 0009 — Verify drives in a private authoritative market service

## Context

Two players on different clients need common listings and balances. Local saves
are editable and cannot supply trusted coins to that market. The pitch calls for
a shared test market, not a public account platform or deployment.

## Decision

Run a native service from the same executable with `--market-server`. A network
`ProfileStore` uses the same garage transaction implementation as the local store.
The operator provisions a finite set of accounts; a 256-bit token from OS
randomness authenticates each account. Credentials are written to a new private
file. There is no registration endpoint or local-save import. Schema 2 records
whether a file belongs to the local game or the market and refuses cross-use.

The protocol is versioned, length-prefixed JSON with a 4 MiB maximum, timeouts,
four connection workers and a bounded queue. All authenticated service operations
are serialized, including login, idle checks and writes. Bind and connect only to
loopback. Another device uses an authenticated SSH tunnel; this raw transport is
not exposed publicly and has no built-in TLS. A public release needs a separate
hosting, account-recovery and abuse-control design.

Before a drive the service issues a persistent ticket containing a random seed
and the owned vehicle configuration. The client quantizes steering to 127 steps
in each direction and records fixed-step steering and boost. At drive end the
service replays these inputs through `SimulationPlugin`, computes coins itself,
and atomically credits the result and marks the ticket completed. It accepts no
client score. It rejects inputs after game over, runs faster than elapsed server
time, unknown tickets and altered repeated submissions. The fixed rate is 64 Hz;
online runs end after 20 minutes to bound recording and verification costs.

One account has one active client session and one active ticket. A new client
login abandons unfinished tickets from an earlier launch. Confirmed credits and
garage receipts remain. Vehicle mutations are rejected during a ticket. Online
restart ends/submits the current drive; the next drive obtains a fresh ticket
through the garage. Local play retains immediate restart.

## Consequences

The two-client experiment supports real ownership transfer without trusting
editable saves or reported distances. `getrandom` is a direct dependency for
credentials/seeds. Client and server must run compatible game/protocol versions.
Offline play and server accounts progress separately. A lost connection is
retried with the same ticket or transaction; a crash may lose an unsubmitted drive.

Reconstructed alternatives: trusting a reported distance permits unlimited coins;
running an always-connected multiplayer simulation adds latency and another game
mode. Bounded replay reuses the headless simulation but can delay other requests
while a long drive is verified. It verifies legal simulated outcomes, not human
play, and is not a general anti-bot system. Test coins can be granted only by the
operator CLI and must never be described as earned progress.

## The tell

`cargo test --locked market::tests` exercises authentication, TCP transfer,
idempotent payout, real-time bounds and the drive lock.
`cargo test --locked recorded_drives` compares recordings for all ten models
with server replay; `cargo test --locked online_garage_start` checks ticket
lifecycle. These do not replace a two-machine SSH test or load testing a public
service. See the [operator guide](../../marketplace.md).
