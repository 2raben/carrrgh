//! Road geometry and continuous random traffic; no rendering or device input.
use bevy::prelude::*;

use crate::pickups::{Pickup, keeps_clear_for};
use crate::player::{PlayerTuning, Position, PreviousPosition};
use crate::run::Run;

pub const ROAD_HALF_WIDTH: f32 = 210.0;
pub const LANES: [f32; 3] = [-140.0, 0.0, 140.0];
pub const CAR_SIZE: Vec2 = Vec2::new(44.0, 78.0);
pub const TRUCK_SIZE: Vec2 = Vec2::new(70.0, 144.0);
pub const SIGNAL_SECONDS: f32 = 0.8;
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

    fn change_seconds(self) -> f32 {
        match self {
            Self::Car => 1.0,
            Self::Truck => 1.4,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LaneChange {
    pub target_lane: usize,
    pub start_x: f32,
    pub target_x: f32,
    pub elapsed: f32,
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

/// Shared admission rule for spawning and reserving an adjacent lane. Requiring
/// equal signed speeds in shared lanes also prevents future rear-end overlaps.
fn traffic_is_safe(
    candidate: (Traffic, Vec2),
    others: &[(Traffic, Vec2)],
    tuning: &PlayerTuning,
) -> bool {
    for (i, &(car, at)) in others.iter().enumerate() {
        if (0..LANES.len()).any(|lane| candidate.0.occupies(lane) && car.occupies(lane))
            && (candidate.0.speed != car.speed || (candidate.1.y - at.y).abs() < TRAFFIC_SPACING)
        {
            return false;
        }
        for &other in &others[i + 1..] {
            if blocks_lanes_for([candidate, (car, at), other], tuning) {
                return false;
            }
        }
    }
    true
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
    let existing: Vec<_> = traffic.iter().map(|(car, p)| (*car, p.0)).collect();
    let kind = if flow.rng.f32() < run.truck_chance() {
        VehicleKind::Truck
    } else {
        VehicleKind::Car
    };
    let offset = (flow.rng.f32() * 2.0 - 1.0) * kind.max_offset();
    let mut lanes = [0, 1, 2];
    flow.rng.shuffle(&mut lanes);
    for lane in lanes {
        let at = Vec2::new(LANES[lane] + offset, SPAWN_Y);
        // Cars already in a lane set its speed, preventing rear-end overlaps.
        let speed = traffic
            .iter()
            .find(|(car, _)| car.occupies(lane))
            .map(|(car, _)| car.speed)
            .unwrap_or_else(|| if flow.rng.bool() { 140.0 } else { -150.0 });
        let mut car = Traffic::new(lane, speed, kind);
        if !traffic_is_safe((car, at), &existing, &tuning) {
            continue;
        }
        // Never close in on a pickup before the player can reach it.
        if pickups
            .iter()
            .any(|p| p.0.x == LANES[lane] && !keeps_clear_for(p.0.y, SPAWN_Y, speed, &tuning))
        {
            continue;
        }
        car.change_cooldown = 0.1 + flow.rng.f32() * 0.35;
        commands.spawn((
            Name::new("Traffic"),
            car,
            Position(at),
            PreviousPosition(at),
        ));
        flow.cooldown = (1.2 + flow.rng.f32() * 0.8) / (1.0 + 0.15 * run.difficulty_step() as f32);
        return;
    }
    // Retry soon if every lane is unsafe; never emit a catch-up batch.
    flow.cooldown = 0.1;
}

pub fn plan_lane_changes(
    time: Res<Time>,
    tuning: Res<PlayerTuning>,
    mut flow: ResMut<TrafficFlow>,
    mut traffic: Query<(Entity, &mut Traffic, &Position)>,
    pickups: Query<&Position, With<Pickup>>,
) {
    // Serial reservations keep simultaneous swaps from invalidating each other.
    // New spawns and pickups also consult these reservations on following ticks.
    if traffic.iter().any(|(_, car, _)| car.change.is_some()) {
        return;
    }
    let snapshot: Vec<_> = traffic.iter().map(|(id, car, p)| (id, *car, p.0)).collect();
    for (id, mut car, position) in &mut traffic {
        car.change_cooldown -= time.delta_secs();
        if car.change_cooldown > 0.0 || !(160.0..=360.0).contains(&position.0.y) {
            continue;
        }
        car.change_cooldown = 0.4 + flow.rng.f32() * 0.8;
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
            });
            if !traffic_is_safe((candidate, position.0), &others, &tuning)
                || pickups.iter().any(|p| {
                    p.0.x == LANES[target]
                        && !keeps_clear_for(p.0.y, position.0.y, car.speed, &tuning)
                })
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
    for (mut car, mut position, mut previous) in &mut traffic {
        previous.0 = position.0;
        position.0.y += (car.speed - run.speed) * time.delta_secs();
        let duration = car.kind.change_seconds();
        if let Some(change) = &mut car.change {
            change.elapsed += time.delta_secs();
            let progress = ((change.elapsed - SIGNAL_SECONDS) / duration).clamp(0.0, 1.0);
            // Smooth start and finish, with no lateral motion during the warning.
            let eased = progress * progress * (3.0 - 2.0 * progress);
            position.0.x = change.start_x + (change.target_x - change.start_x) * eased;
            if progress >= 1.0 {
                car.lane = change.target_lane;
                car.change = None;
                car.change_cooldown = 2.0;
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
