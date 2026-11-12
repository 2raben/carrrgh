# Car Drive

A three-lane arcade driving game seen from above. The player drives automatically,
steers through traffic in both directions and chooses when to spend boost energy.
Three hearts allow mistakes; the distance reached gives the next run a target.

The first prototype replaces the starter platformer and implements the
[Car Drive pitch](pitches/car-drive/pitch--car-movements.md), based on the original
hand-drawn idea. It has one straight road, continuous random traffic, a rechargeable
boost, damage protection, game over and immediate restart. The
[power-ups and shields pitch](pitches/power-ups/pitch--power-ups-and-shields.md)
adds rare hearts that restore a life and a shield that absorbs the next hit. The
[coins and player profile pitch](pitches/coins/pitch--coins-and-player-profile.md)
turns driven distance into coins that are saved for a local player.

The next useful work is play-testing: steering feel, reaction time, traffic
readability and whether boost creates interesting choices. Additional vehicles,
tracks, upgrades, spending coins and saved scores are not implemented. Shape any such
extension separately before implementing it.
