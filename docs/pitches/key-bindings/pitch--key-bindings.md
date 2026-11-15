# Problem

Car Drive's keyboard controls are fixed. Players cannot choose keys that suit
their preferred layout, and boost is only available on Space. Automatic forward
motion also gives players no way to slow down when approaching traffic.

# Solution

Let players configure keyboard bindings and add a brake action.

**Status: implemented.** Braking takes priority over boost, recharges energy,
and sets speed to 75% of cruise; releasing it restores cruise immediately.
Bindings support 1–4 keys per action, with conflicts rejected and G/Escape reserved.
Local settings persistence and replay compatibility are recorded in
[ADR 0012](../../architecture/decisions/0012-persist-local-key-bindings-and-replay-braking.md).
The current controls are described in the
[architecture documentation](../../architecture/README.md).

Provide a key-bindings screen reachable from the garage. Show each gameplay
action and its assigned keys. Players can select an action, add or replace a
binding by pressing a key, remove an alternative binding, and restore defaults.
Allow several keys for the same action, so arrow-key and WASD players can both
use the defaults immediately.

The [editable breadboard](pitch--key-bindings.breadboard.excalidraw) shows the
settings and key-capture flow, including cancelling a change.

| Action | Default keys |
| --- | --- |
| Steer left | `Arrow Left`, `A` |
| Steer right | `Arrow Right`, `D` |
| Boost | `Space`, `Arrow Up`, `W` |
| Brake | `Arrow Down`, `S` |
| Restart / end drive | `R`, `Backspace` |

Holding any assigned boost key uses the existing boost behavior. Holding brake
slows the car below its normal cruising speed; releasing it lets the car return
to cruising speed. Steering remains available while braking. Choose braking
strength and the return to cruising speed through play-testing, keeping forward
motion and the arcade feel.

Keep chosen bindings across launches on native and browser builds, and show the
active bindings in control hints. Capturing a key must not also steer, boost,
brake or activate a garage action. Players can cancel capture without changing
the existing binding and restore defaults if their configuration is awkward.

# Rabbit Holes

- **Conflicting keys.** Make conflicts visible when assigning a key already
  used by another gameplay action. Avoid silently triggering two actions or
  leaving the player without a usable binding. Menu navigation and text entry
  need their own input context, especially for the arrow keys.
- **Boost and brake together.** Define one predictable precedence before
  implementation, including whether boost energy drains or recharges while
  braking. Do not let opposite inputs produce contradictory speed effects.
- **Braking balance.** Slower movement changes traffic encounters, distance
  gain and pickup timing. Test these together rather than choosing a speed
  reduction in isolation. Avoid expanding this into realistic vehicle physics.
- **Layouts and browser focus.** Keep key labels understandable on the player's
  layout, and check that supported bindings work while the browser canvas has
  focus without scrolling the page.
- **Replay parity.** The private market verifies drives by replaying controls.
  Braking must be represented consistently in live play and verification;
  physical key choices should not affect the resulting simulation intent.

# No-Gos

- No controller remapping, touch controls, key chords or macros in this pitch.
- No reverse gear, handbrake, drifting or full vehicle physics.
- No cloud synchronization or sharing of binding presets.
- No redesign of the garage or changes to menu-navigation bindings.
