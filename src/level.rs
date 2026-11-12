//! Road geometry and continuous random traffic; no rendering or device input.
use bevy::prelude::*;

use crate::pickups::{Pickup, keeps_clear};
use crate::player::{PlayerTuning, Position, PreviousPosition};
use crate::run::Run;

pub const ROAD_HALF_WIDTH: f32 = 210.0;
pub const LANES: [f32; 3] = [-140.0, 0.0, 140.0];
pub const CAR_SIZE: Vec2 = Vec2::new(44.0, 78.0);
pub const PLAYER_START: Vec2 = Vec2::new(140.0, -230.0);
pub const SPAWN_Y: f32 = 480.0;
pub const DESPAWN_Y: f32 = -470.0;
pub const TRAFFIC_SPACING: f32 = 180.0;
pub const ESCAPE_SPACING: f32 = 360.0;

/// Positive speed travels in the player's direction; negative is oncoming.
#[derive(Component, Clone, Copy, Debug)]
pub struct Traffic {
    pub speed: f32,
    pub hit: bool,
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
pub fn blocks_all_lanes(cars: [(Vec2, f32); 3], cruise_speed: f32) -> bool {
    if cars[0].0.x == cars[1].0.x || cars[0].0.x == cars[2].0.x || cars[1].0.x == cars[2].0.x {
        return false;
    }
    let mut from = 0.0_f32;
    let mut until = cars
        .iter()
        .map(|(at, speed)| (at.y - DESPAWN_Y).max(0.0) / (cruise_speed - speed))
        .fold(f32::INFINITY, f32::min);
    // Intersect the time intervals when each pair is within ESCAPE_SPACING.
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        let gap = cars[a].0.y - cars[b].0.y;
        let relative_speed = cars[a].1 - cars[b].1;
        if relative_speed == 0.0 {
            if gap.abs() > ESCAPE_SPACING {
                return false;
            }
        } else {
            let first = (-ESCAPE_SPACING - gap) / relative_speed;
            let last = (ESCAPE_SPACING - gap) / relative_speed;
            from = from.max(first.min(last));
            until = until.min(first.max(last));
        }
        if from > until {
            return false;
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
    let mut lanes = LANES;
    flow.rng.shuffle(&mut lanes);
    for x in lanes {
        let at = Vec2::new(x, SPAWN_Y);
        if traffic
            .iter()
            .any(|(_, p)| p.0.x == x && SPAWN_Y - p.0.y < TRAFFIC_SPACING)
        {
            continue;
        }
        // Cars already in a lane set its speed, preventing rear-end overlaps.
        let speed = traffic
            .iter()
            .find(|(_, p)| p.0.x == x)
            .map(|(car, _)| car.speed)
            .unwrap_or_else(|| if flow.rng.bool() { 140.0 } else { -150.0 });
        if traffic.iter_combinations::<2>().any(|[(a, pa), (b, pb)]| {
            blocks_all_lanes(
                [(at, speed), (pa.0, a.speed), (pb.0, b.speed)],
                tuning.cruise_speed,
            )
        }) {
            continue;
        }
        // Never close in on a pickup before the player can reach it.
        if pickups
            .iter()
            .any(|p| p.0.x == x && !keeps_clear(p.0.y, SPAWN_Y, speed, tuning.cruise_speed))
        {
            continue;
        }
        commands.spawn((
            Name::new("Traffic"),
            Traffic { speed, hit: false },
            Position(at),
            PreviousPosition(at),
        ));
        flow.cooldown = (1.2 + flow.rng.f32() * 0.8) / (1.0 + 0.15 * run.difficulty_step() as f32);
        return;
    }
    // Retry soon if every lane is unsafe; never emit a catch-up batch.
    flow.cooldown = 0.1;
}

pub fn move_traffic(
    time: Res<Time>,
    run: Res<Run>,
    mut traffic: Query<(&Traffic, &mut Position, &mut PreviousPosition)>,
) {
    for (car, mut position, mut previous) in &mut traffic {
        previous.0 = position.0;
        position.0.y += (car.speed - run.speed) * time.delta_secs();
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
