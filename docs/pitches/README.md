# Pitches

A pitch shapes a substantial feature before it is built: the problem, a rough
solution, the risks, and what is deliberately left out. Small fixes and
documentation corrections do not need one. See
[CONTRIBUTING.md](../../CONTRIBUTING.md#shaping-work) for the workflow.

Save each pitch as `docs/pitches/<topic>/pitch--<slug>.md`, and keep breadboards,
fat-marker sketches (for example Excalidraw files), and reference images beside
it. The [Car Drive pitch](car-drive/pitch--car-movements.md) includes a reference
sketch and an editable Excalidraw drawing.

## Template

Copy this outline into a new pitch:

```markdown
# Problem

What is wrong or missing, and for whom? Describe the situation that makes this
worth doing now, without proposing the solution yet.

# Solution

The rough shape of the answer: the elements, how the player meets them, and how
they connect. Link breadboards or sketches kept beside this file. Stay at the
level of detail that leaves room for the implementation to decide.

# Security

Network, storage, external content, or untrusted input this introduces, and how
it is contained, or why no security concern applies.

# Rabbit Holes

Details that could swallow the work, and the decision that avoids each one.

# No-Gos

What this pitch explicitly does not include.
```

## Index

| Pitch | Status |
| --- | --- |
| [Car Drive — car movements](car-drive/pitch--car-movements.md) | implemented; replaces the starter platformer |
| [Power-ups and shields](power-ups/pitch--power-ups-and-shields.md) | implemented |
| [Traffic — lane changes and trucks](traffic/pitch--lane-changes-and-trucks.md) | implemented |
| [Coins and player profile](coins/pitch--coins-and-player-profile.md) | implemented |
| [Garage, vehicle tuning and marketplace](vehicles/pitch--garage-tuning-and-marketplace.md) | implemented as a local garage and private shared test market |
| [Endless difficulty and mixed traffic](traffic/pitch--endless-difficulty-and-mixed-traffic.md) | implemented |
| [Browser game](browser-game/pitch--browser-game.md) | implemented; Pages must use GitHub Actions as its source |
| [Key bindings](key-bindings/pitch--key-bindings.md) | implemented |
