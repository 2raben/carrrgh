# Problem

Traffic stays exactly on lane centres, leaving the lane markings permanently
safe. Later runs also lack new vehicle types and enough distinct difficulty steps.

# Solution

Let cars keep a random lateral offset inside their lane and change to an adjacent
lane after a visible turn signal. Reserve both lanes while signalling and moving,
using the existing spacing, wall prediction and pickup clearance rules. Keep the
motion continuous and swept collisions aware of each vehicle's actual size.

Use ten difficulty levels, one per 1,000 metres through 9,000 metres, with bounded
speed and spawn frequency. Show the level in the HUD. From 4,000 metres, mix in
trucks with a visibly wider body, longer cargo bed and correspondingly larger
collision box. Trucks change lanes more slowly.

# Security

All behavior is local simulation and procedural drawing; no new external content,
network access, persistence or dependencies are needed.

# Rabbit Holes

Use conservative lane reservations instead of a general traffic AI or path planner.
Check warning time, manoeuvre readability and usable gaps in the running game;
seeded tests establish safety invariants but cannot establish final balance.

# No-Gos

No physics engine, traffic-to-traffic impacts, articulated trailers, new controls,
road layouts or menus. Difficulty values remain play-test tuning.
