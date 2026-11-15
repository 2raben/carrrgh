# Private marketplace test

The garage works locally by default. The shared marketplace is a separate,
operator-provisioned test service. Two clients can buy each other's listed cars;
the marketplace pays the seller immediately and then owns the listing. Local
coins and vehicles are never imported. No public service is deployed by this repo.

Run these commands from the repository root, inside `nix-shell` on NixOS.

## Start a shared test

Build once and create two accounts in a **new, separate** database:

```sh
cargo build --locked
target/debug/bevy-template --market-provision /tmp/car-drive-market/market.sqlite3 Alice /tmp/car-drive-alice.json 3000000
target/debug/bevy-template --market-provision /tmp/car-drive-market/market.sqlite3 Bob /tmp/car-drive-bob.json 3000000
target/debug/bevy-template --market-server /tmp/car-drive-market/market.sqlite3
```

The optional last provisioning argument grants test coins. Omit it to start with
zero earned coins and a free Corsa. Account creation is an operator action; the
network has no registration or grant endpoint. Keep credential files private.
Existing credential files are not overwritten. On Unix they and the market
database are restricted to their owner.

In two other terminals, launch the clients:

```sh
CAR_DRIVE_MARKET_CONFIG=/tmp/car-drive-alice.json target/debug/bevy-template
CAR_DRIVE_MARKET_CONFIG=/tmp/car-drive-bob.json target/debug/bevy-template
```

Both open their own garage with the shared market. Alice buys a second vehicle,
changes its color and tuning, then chooses **Sell now** and confirms the displayed
instant payout. Bob opens **Marketplace**, refreshes, and buys that vehicle.
Its color and all purchased upgrades transfer. Alice receives no second payout.
Restart both clients and verify their balances and collections persist.

The server address defaults to `127.0.0.1:47831`. An optional address follows the
database argument for `--market-server`; update the `address` field in each
credential JSON file to match. `CAR_DRIVE_DATA_DIR` is ignored for network accounts.
Use a different credential per player; logging in elsewhere with the same
credential replaces the old session and abandons its unsubmitted drive.

## A second machine

Transfer only that player's credential file through a trusted channel. Run an
SSH tunnel from the player's machine to the server host:

```sh
ssh -N -L 47831:127.0.0.1:47831 user@server-host
```

Keep the client's configured address at `127.0.0.1:47831`. The service deliberately
refuses non-loopback binds and the client refuses non-loopback connections: the
tunnel provides encryption and host authentication. Use the same game build on
both machines. The experiment does not include public hosting, automatic account
recovery, or a browser client.

## Playing and recovery

Up/Down moves menu focus; Enter or controller South activates it. Tab or the
controller's right shoulder cycles sections. Mouse buttons work too. G/Start
starts a drive from the garage or opens it after game over. Escape/East cancels a
confirmation. Escape outside the garage quits the game; the window close button
also works in the garage.

Online R/Select ends and submits the current drive. Open the garage with G/Start
to start the next one. Online drives have a 20-minute limit. The server issues
the seed/configuration and replays recorded input to compute coins; it ignores
client balance/score claims. It also checks elapsed server time. A drive's live
coin count remains provisional until confirmation.

During a connection failure the garage keeps the same pending transaction.
Choose **Retry / refresh safely** after reconnecting. This queries/retries that
operation without another purchase or payout. Pending drive credits must finish
before shopping or starting another online run. Closing the app attempts a final
submission; a crash can lose unsubmitted input. Previously confirmed purchases
and coins remain on the server.

## Validation boundaries

Automated tests cover schema migration, realm separation, grants, prices, ownership,
concurrent buyers, authenticated TCP trading, repeated requests and drive replay
for all ten models. Graphical testing covers the local garage. A separate
two-machine SSH session and physical controller play test are still useful before
inviting a larger test group. The serial service and bounded replay are intended
for this private experiment, not a publicly exposed marketplace.

## Input compatibility

Protocol version 2 records steering, boost and brake for each fixed tick. Update
client and server together; version-1 clients are rejected. Keyboard bindings are
local preferences and are not sent to the market. Braking is verified by the same
simulation used during play.
