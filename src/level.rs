//! Road geometry and continuous random traffic; no rendering or device input.
use bevy::prelude::*;

use crate::difficulty::Difficulty;
use crate::pickups::{Pickup, keeps_clear_for};
use crate::player::{PlayerTuning, Position, PreviousPosition};
use crate::run::Run;

pub const ROAD_HALF_WIDTH: f32 = 210.0;
pub const LANES: [f32; 3] = [-140.0, 0.0, 140.0];
pub const CAR_SIZE: Vec2 = Vec2::new(44.0, 78.0);
pub const TRUCK_SIZE: Vec2 = Vec2::new(70.0, 144.0);
pub const SIGNAL_SECONDS: f32 = 0.8;
/// Evasions resolve head-on meetings, so they warn and cross faster.
pub const EVASION_SIGNAL_SECONDS: f32 = 0.5;
/// Covers fixed-step quantisation when an evasion releases its source lane.
const EVASION_SLACK: f32 = 0.05;
/// Signed traffic speeds: positive travels in the player's direction.
pub const SAME_DIRECTION_SPEED: f32 = 140.0;
pub const ONCOMING_SPEED: f32 = -150.0;
/// From mixing levels on, a longer run of spawns in one direction forces the
/// other one, so dense same-direction traffic cannot lock oncoming traffic out.
pub const MAX_DIRECTION_STREAK: u32 = 4;
pub const PLAYER_START: Vec2 = Vec2::new(140.0, -230.0);
pub const SPAWN_Y: f32 = 480.0;
pub const DESPAWN_Y: f32 = -470.0;
pub const TRAFFIC_SPACING: f32 = 180.0;
pub const ESCAPE_SPACING: f32 = 360.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VehicleKind {
    Car,
    Truck,
}

impl VehicleKind {
    pub fn size(self) -> Vec2 {
        match self {
            Self::Car => CAR_SIZE,
            Self::Truck => TRUCK_SIZE,
        }
    }

    pub fn max_offset(self) -> f32 {
        match self {
            Self::Car => 36.0,
            Self::Truck => 25.0,
        }
    }

    /// Lateral crossing time after the signal phase.
    fn change_seconds(self, evasive: bool) -> f32 {
        match (self, evasive) {
            (Self::Car, false) => 1.0,
            (Self::Truck, false) => 1.4,
            (Self::Car, true) => 0.6,
            (Self::Truck, true) => 0.8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LaneChange {
    pub target_lane: usize,
    pub start_x: f32,
    pub target_x: f32,
    pub elapsed: f32,
    /// An oncoming vehicle giving way to same-direction traffic in its lane.
    pub evasive: bool,
}

impl LaneChange {
    pub fn signal_seconds(&self) -> f32 {
        if self.evasive {
            EVASION_SIGNAL_SECONDS
        } else {
            SIGNAL_SECONDS
        }
    }
}

/// Lane identity is independent of lateral position. A manoeuvre reserves both
/// lanes from the first signal until the vehicle has reached its target offset.
#[derive(Component, Clone, Copy, Debug)]
pub struct Traffic {
    /// Positive speed travels in the player's direction; negative is oncoming.
    pub speed: f32,
    pub hit: bool,
    pub kind: VehicleKind,
    pub lane: usize,
    pub change: Option<LaneChange>,
    pub change_cooldown: f32,
}

impl Traffic {
    pub fn new(lane: usize, speed: f32, kind: VehicleKind) -> Self {
        Self {
            speed,
            hit: false,
            kind,
            lane,
            change: None,
            change_cooldown: 0.0,
        }
    }

    /// Seconds until an active manoeuvre releases its source lane.
    fn change_remaining(&self) -> f32 {
        self.change.map_or(0.0, |change| {
            change.signal_seconds() + self.kind.change_seconds(change.evasive) - change.elapsed
        })
    }

    pub fn occupies(&self, lane: usize) -> bool {
        self.lane == lane || self.change.is_some_and(|c| c.target_lane == lane)
    }

    /// Lit side in road coordinates, including the stationary warning phase.
    pub fn signal_side(&self) -> Option<f32> {
        self.change
            .filter(|c| c.elapsed % 0.5 < 0.25)
            .map(|c| (LANES[c.target_lane] - LANES[self.lane]).signum())
    }
}

/// Filter for everything that drives or lies on the road besides the player.
pub type OnRoad = Or<(With<Traffic>, With<Pickup>)>;

#[derive(Resource, Default)]
pub struct TrafficFlow {
    pub cooldown: f32,
    pub rng: fastrand::Rng,
    /// Signed speed of the latest spawn and how many spawns in a row used it.
    pub last_speed: f32,
    pub streak: u32,
}

/// Whether three cars can pack all lanes into a short longitudinal band.
/// Relative car spacing is independent of player speed. The initial cruise
/// speed gives the longest remaining lifetime, including later boost releases.
#[cfg(test)]
pub fn blocks_all_lanes(cars: [(Traffic, Vec2); 3], cruise_speed: f32) -> bool {
    blocks_lanes_for(
        cars,
        &PlayerTuning {
            cruise_speed,
            ..default()
        },
    )
}

fn blocks_lanes_for(cars: [(Traffic, Vec2); 3], tuning: &PlayerTuning) -> bool {
    let cruise_speed = tuning.cruise_speed;
    let covers_road = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ]
    .iter()
    .any(|lanes| (0..3).all(|i| cars[i].0.occupies(lanes[i])));
    if !covers_road {
        return false;
    }
    let spacing = ESCAPE_SPACING * (420. / tuning.steer_speed).max(1.)
        + (tuning.size.y - CAR_SIZE.y).max(0.)
        + (tuning.size.x - CAR_SIZE.x).max(0.)
        + cars
            .iter()
            .map(|(car, _)| car.kind.size().y - CAR_SIZE.y)
            .fold(0.0_f32, f32::max);
    let mut from = 0.0_f32;
    let mut until = cars
        .iter()
        .map(|(car, at)| (at.y - DESPAWN_Y).max(0.0) / (cruise_speed - car.speed))
        .fold(f32::INFINITY, f32::min);
    // Intersect the time intervals when each pair is within ESCAPE_SPACING.
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        let gap = cars[a].1.y - cars[b].1.y;
        let relative_speed = cars[a].0.speed - cars[b].0.speed;
        if relative_speed == 0.0 {
            if gap.abs() > spacing {
                return false;
            }
        } else {
            let first = (-spacing - gap) / relative_speed;
            let last = (spacing - gap) / relative_speed;
            from = from.max(first.min(last));
            until = until.min(first.max(last));
        }
        if from > until {
            return false;
        }
    }
    true
}

/// Whether two vehicles may share a lane now and until one of them leaves it.
/// Same-direction pairs keep a constant gap. Only levels that mix directions
/// allow anything else: diverging pairs, which only separate, and head-on pairs.
/// A head-on pair is allowed when the lower, same-direction vehicle leaves the
/// road first, or when the oncoming vehicle already evades out of the only
/// shared lane and releases it before the gap closes below `TRAFFIC_SPACING`.
fn lane_sharing_is_safe(
    a: (Traffic, Vec2),
    b: (Traffic, Vec2),
    tuning: &PlayerTuning,
    mixing: bool,
) -> bool {
    let shared = |lane| a.0.occupies(lane) && b.0.occupies(lane);
    if !(0..LANES.len()).any(shared) {
        return true;
    }
    if a.0.speed == b.0.speed {
        return (a.1.y - b.1.y).abs() >= TRAFFIC_SPACING;
    }
    if !mixing {
        return false;
    }
    let (same, oncoming) = if a.0.speed > b.0.speed {
        (a, b)
    } else {
        (b, a)
    };
    // Positive while the oncoming vehicle is ahead and the two close in.
    let ahead = oncoming.1.y - same.1.y;
    if ahead <= 0.0 {
        return -ahead >= TRAFFIC_SPACING;
    }
    let closing = same.0.speed - oncoming.0.speed;
    // The initial cruise speed gives the lower vehicle its longest lifetime.
    let exit_speed = tuning.cruise_speed - same.0.speed;
    let lifetime = if exit_speed > 0.0 {
        (same.1.y - DESPAWN_Y).max(0.0) / exit_speed
    } else {
        f32::INFINITY
    };
    if ahead - closing * lifetime >= TRAFFIC_SPACING {
        return true;
    }
    // No evasion chains: the target lane must not hold the same-direction vehicle.
    oncoming.0.change.is_some_and(|change| {
        change.evasive
            && !same.0.occupies(change.target_lane)
            && (0..LANES.len()).all(|lane| lane == oncoming.0.lane || !shared(lane))
            && ahead - closing * (oncoming.0.change_remaining() + EVASION_SLACK) >= TRAFFIC_SPACING
    })
}

/// Shared admission rule for spawning and reserving lanes: lane sharing plus
/// the predictive three-lane wall check against every pair of other vehicles.
pub(crate) fn traffic_is_safe(
    candidate: (Traffic, Vec2),
    others: &[(Traffic, Vec2)],
    tuning: &PlayerTuning,
    difficulty: &Difficulty,
) -> bool {
    let mixing = difficulty.mix_chance > 0.0;
    for (i, &other) in others.iter().enumerate() {
        if !lane_sharing_is_safe(candidate, other, tuning, mixing) {
            return false;
        }
        for &third in &others[i + 1..] {
            if blocks_lanes_for([candidate, other, third], tuning) {
                return false;
            }
        }
    }
    true
}

/// Never close in on a pickup in any reserved lane before the player can reach it.
fn keeps_pickups_clear(car: &Traffic, y: f32, pickups: &[Vec2], tuning: &PlayerTuning) -> bool {
    pickups.iter().all(|p| {
        !(0..LANES.len()).any(|lane| car.occupies(lane) && p.x == LANES[lane])
            || keeps_clear_for(p.y, y, car.speed, tuning)
    })
}

/// Lanes next to `lane`, in random order.
fn adjacent_lanes(lane: usize, rng: &mut fastrand::Rng) -> Vec<usize> {
    let mut lanes: Vec<_> = (0..LANES.len())
        .filter(|&l| l.abs_diff(lane) == 1)
        .collect();
    rng.shuffle(&mut lanes);
    lanes
}

/// Admits a vehicle unchanged or, for oncoming traffic when the level allows
/// it, together with an evasion reserved from the start.
fn admit(
    car: Traffic,
    at: Vec2,
    others: &[(Traffic, Vec2)],
    pickups: &[Vec2],
    tuning: &PlayerTuning,
    difficulty: &Difficulty,
    rng: &mut fastrand::Rng,
) -> Option<Traffic> {
    let clear = |car: &Traffic| {
        traffic_is_safe((*car, at), others, tuning, difficulty)
            && keeps_pickups_clear(car, at.y, pickups, tuning)
    };
    if clear(&car) {
        return Some(car);
    }
    if !difficulty.evasions || car.speed > 0.0 {
        return None;
    }
    for target in adjacent_lanes(car.lane, rng) {
        let evading = Traffic {
            change: Some(LaneChange {
                target_lane: target,
                start_x: at.x,
                target_x: LANES[target] + (rng.f32() * 2.0 - 1.0) * car.kind.max_offset(),
                elapsed: 0.0,
                evasive: true,
            }),
            ..car
        };
        if clear(&evading) {
            return Some(evading);
        }
    }
    None
}

pub fn spawn_traffic(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    tuning: Res<PlayerTuning>,
    mut flow: ResMut<TrafficFlow>,
    traffic: Query<(&Traffic, &Position)>,
    pickups: Query<&Position, With<Pickup>>,
) {
    flow.cooldown -= time.delta_secs();
    if flow.cooldown > 0.0 {
        return;
    }
    let difficulty = run.difficulty();
    let existing: Vec<_> = traffic.iter().map(|(car, p)| (*car, p.0)).collect();
    let pickups: Vec<_> = pickups.iter().map(|p| p.0).collect();
    let kind = if flow.rng.f32() < difficulty.truck_chance {
        VehicleKind::Truck
    } else {
        VehicleKind::Car
    };
    let offset = (flow.rng.f32() * 2.0 - 1.0) * kind.max_offset();
    let mut lanes = [0, 1, 2];
    flow.rng.shuffle(&mut lanes);
    // Rolled once per spawn, and only when an occupied lane makes it matter.
    let mut mix = None;
    for lane in lanes {
        let at = Vec2::new(LANES[lane] + offset, SPAWN_Y);
        // The nearest vehicle in a lane sets its direction unless this spawn mixes.
        let nearest = existing
            .iter()
            .filter(|(car, _)| car.occupies(lane))
            .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
            .map(|(car, _)| car.speed);
        let speed = nearest.unwrap_or_else(|| {
            if flow.rng.bool() {
                SAME_DIRECTION_SPEED
            } else {
                ONCOMING_SPEED
            }
        });
        let mixes = nearest.is_some()
            && difficulty.mix_chance > 0.0
            && *mix.get_or_insert_with(|| flow.rng.f32() < difficulty.mix_chance);
        let opposite = if speed > 0.0 {
            ONCOMING_SPEED
        } else {
            SAME_DIRECTION_SPEED
        };
        let forced = (difficulty.mix_chance > 0.0 && flow.streak >= MAX_DIRECTION_STREAK)
            .then(|| -flow.last_speed.signum());
        // A rejected mix falls back to the lane's own direction.
        let speeds = if mixes {
            &[opposite, speed][..]
        } else {
            &[speed][..]
        };
        let speeds = match forced {
            Some(sign) if sign > 0.0 => &[SAME_DIRECTION_SPEED][..],
            Some(_) => &[ONCOMING_SPEED][..],
            None => speeds,
        };
        for &speed in speeds {
            let car = Traffic::new(lane, speed, kind);
            let Some(mut car) = admit(
                car,
                at,
                &existing,
                &pickups,
                &tuning,
                &difficulty,
                &mut flow.rng,
            ) else {
                continue;
            };
            car.change_cooldown = 0.1 + flow.rng.f32() * 0.35;
            commands.spawn((
                Name::new("Traffic"),
                car,
                Position(at),
                PreviousPosition(at),
            ));
            flow.cooldown = difficulty.spawn_delay(flow.rng.f32());
            flow.streak = if speed == flow.last_speed {
                flow.streak + 1
            } else {
                1
            };
            flow.last_speed = speed;
            return;
        }
    }
    // Retry soon if every lane is unsafe; never emit a catch-up batch.
    flow.cooldown = 0.1;
}

pub fn plan_lane_changes(
    time: Res<Time>,
    run: Res<Run>,
    tuning: Res<PlayerTuning>,
    mut flow: ResMut<TrafficFlow>,
    mut traffic: Query<(Entity, &mut Traffic, &Position)>,
    pickups: Query<&Position, With<Pickup>>,
) {
    let difficulty = run.difficulty();
    // At most one reservation per tick, checked against all active ones, keeps
    // simultaneous swaps from invalidating each other. Evasions were reserved
    // at admission and do not count towards the voluntary limit.
    let voluntary = traffic
        .iter()
        .filter(|(_, car, _)| car.change.is_some_and(|change| !change.evasive))
        .count();
    if voluntary >= difficulty.simultaneous_changes {
        return;
    }
    let snapshot: Vec<_> = traffic.iter().map(|(id, car, p)| (id, *car, p.0)).collect();
    let pickups: Vec<_> = pickups.iter().map(|p| p.0).collect();
    for (id, mut car, position) in &mut traffic {
        if car.change.is_some() {
            continue;
        }
        car.change_cooldown -= time.delta_secs();
        if car.change_cooldown > 0.0 || !(160.0..=360.0).contains(&position.0.y) {
            continue;
        }
        car.change_cooldown = difficulty.retry_delay(flow.rng.f32());
        let others: Vec<_> = snapshot
            .iter()
            .filter(|(other, _, _)| *other != id)
            .map(|(_, car, p)| (*car, *p))
            .collect();
        let mut targets = [0, 1, 2];
        flow.rng.shuffle(&mut targets);
        for target in targets {
            if car.lane.abs_diff(target) != 1 {
                continue;
            }
            let mut candidate = *car;
            candidate.change = Some(LaneChange {
                target_lane: target,
                start_x: position.0.x,
                target_x: LANES[target] + (flow.rng.f32() * 2.0 - 1.0) * car.kind.max_offset(),
                elapsed: 0.0,
                evasive: false,
            });
            if !traffic_is_safe((candidate, position.0), &others, &tuning, &difficulty)
                || !keeps_pickups_clear(&candidate, position.0.y, &pickups, &tuning)
            {
                continue;
            }
            *car = candidate;
            return;
        }
    }
}

pub fn move_traffic(
    time: Res<Time>,
    run: Res<Run>,
    mut traffic: Query<(&mut Traffic, &mut Position, &mut PreviousPosition)>,
) {
    let cooldown = run.difficulty().change_cooldown;
    for (mut car, mut position, mut previous) in &mut traffic {
        previous.0 = position.0;
        position.0.y += (car.speed - run.speed) * time.delta_secs();
        let kind = car.kind;
        if let Some(change) = &mut car.change {
            let duration = kind.change_seconds(change.evasive);
            change.elapsed += time.delta_secs();
            let progress = ((change.elapsed - change.signal_seconds()) / duration).clamp(0.0, 1.0);
            // Smooth start and finish, with no lateral motion during the warning.
            let eased = progress * progress * (3.0 - 2.0 * progress);
            position.0.x = change.start_x + (change.target_x - change.start_x) * eased;
            if progress >= 1.0 {
                car.lane = change.target_lane;
                car.change = None;
                car.change_cooldown = cooldown;
            }
        }
    }
}

/// Removes traffic and pickups that have left the bottom of the view.
pub fn remove_offscreen(mut commands: Commands, road_objects: Query<(Entity, &Position), OnRoad>) {
    for (entity, position) in &road_objects {
        if position.0.y < DESPAWN_Y {
            commands.entity(entity).despawn();
        }
    }
}
