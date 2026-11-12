//! Boost, distance, swept collisions, and the lifecycle of a drive.
use bevy::prelude::*;

use crate::level::{CAR_SIZE, OnRoad, PLAYER_START, Traffic, TrafficFlow};
use crate::pickups::PickupFlow;
use crate::player::{Controls, Player, PlayerTuning, Position, PreviousPosition};

pub const MAX_LIVES: u8 = 3;

#[derive(Resource, Clone, Debug)]
pub struct Run {
    pub lives: u8,
    pub distance: f64,
    pub previous_distance: f64,
    pub energy: f32,
    pub speed: f32,
    pub boosting: bool,
    /// Empty tanks must be released before boost can be used again.
    pub exhausted: bool,
    pub protection: f32,
    /// At most one shield is held; it absorbs the next collision.
    pub shield: bool,
    /// Whether the most recent hit was absorbed by a shield, for presentation.
    pub shield_absorbed: bool,
    pub finished: bool,
    pub just_restarted: bool,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            lives: MAX_LIVES,
            distance: 0.0,
            previous_distance: 0.0,
            energy: 1.0,
            speed: 0.0,
            boosting: false,
            exhausted: false,
            protection: 0.0,
            shield: false,
            shield_absorbed: false,
            finished: false,
            just_restarted: false,
        }
    }
}

impl Run {
    /// Ten world units per metre; cap progression to preserve reaction time.
    pub fn difficulty_step(&self) -> usize {
        (self.distance / 10_000.0).floor().clamp(0.0, 9.0) as usize
    }

    pub fn truck_chance(&self) -> f32 {
        if self.distance < 40_000.0 {
            0.0
        } else if self.distance < 70_000.0 {
            0.2
        } else {
            0.35
        }
    }
}

pub fn driving(run: Res<Run>) -> bool {
    !run.finished && !run.just_restarted
}

pub fn restart_run(
    mut commands: Commands,
    mut controls: ResMut<Controls>,
    mut run: ResMut<Run>,
    mut flow: ResMut<TrafficFlow>,
    mut pickup_flow: ResMut<PickupFlow>,
    road_objects: Query<Entity, OnRoad>,
    mut players: Query<(&mut Position, &mut PreviousPosition), With<Player>>,
) {
    run.just_restarted = std::mem::take(&mut controls.restart);
    if !run.just_restarted {
        return;
    }
    for entity in &road_objects {
        commands.entity(entity).despawn();
    }
    for (mut position, mut previous) in &mut players {
        position.0 = PLAYER_START;
        previous.0 = PLAYER_START;
    }
    *controls = Controls::default();
    *flow = TrafficFlow::default();
    *pickup_flow = PickupFlow::default();
    *run = Run {
        just_restarted: true,
        ..default()
    };
}

pub fn advance_run(
    time: Res<Time>,
    controls: Res<Controls>,
    tuning: Res<PlayerTuning>,
    mut run: ResMut<Run>,
) {
    let dt = time.delta_secs();
    run.protection = (run.protection - dt).max(0.0);
    if !controls.boost {
        run.exhausted = false;
    }
    run.boosting = controls.boost && !run.exhausted && run.energy > 0.0;
    // A partially full final step receives only its available share of boost.
    let boosted_fraction = if run.boosting {
        (run.energy / (tuning.boost_drain * dt)).min(1.0)
    } else {
        0.0
    };
    if run.boosting {
        run.energy = (run.energy - tuning.boost_drain * dt).max(0.0);
        if run.energy == 0.0 {
            run.exhausted = true;
        }
    } else {
        run.energy = (run.energy + tuning.boost_recharge * dt).min(1.0);
    }
    let speed_multiplier = 1.0 + 0.05 * run.difficulty_step() as f32;
    run.speed = (tuning.cruise_speed
        + (tuning.boost_speed - tuning.cruise_speed) * boosted_fraction)
        * speed_multiplier;
    run.previous_distance = run.distance;
    run.distance += f64::from(run.speed * dt);
}

/// Segment versus the Minkowski-expanded car box, in relative coordinates.
/// Both cars may move during a tick: checking only their endpoints misses hits.
pub fn swept_hit(start: Vec2, end: Vec2, half_size: Vec2) -> bool {
    if start.abs().cmple(half_size).all() {
        return true;
    }
    let delta = end - start;
    let Ok(direction) = Dir2::new(delta) else {
        return false;
    };
    RayCast2d::new(start, direction, delta.length())
        .aabb_intersection_at(&Aabb2d::new(Vec2::ZERO, half_size))
        .is_some()
}

pub fn collide(
    tuning: Res<PlayerTuning>,
    mut run: ResMut<Run>,
    players: Query<(&Position, &PreviousPosition), With<Player>>,
    mut traffic: Query<(&Position, &PreviousPosition, &mut Traffic)>,
) {
    if run.protection > 0.0 {
        return;
    }
    let Ok((player, previous_player)) = players.single() else {
        return;
    };
    for (position, previous, mut car) in &mut traffic {
        if !car.hit
            && swept_hit(
                previous.0 - previous_player.0,
                position.0 - player.0,
                (CAR_SIZE + car.kind.size()) * 0.5,
            )
        {
            car.hit = true;
            // A shield replaces only the heart loss; the hit still grants protection.
            run.shield_absorbed = std::mem::take(&mut run.shield);
            if !run.shield_absorbed {
                run.lives = run.lives.saturating_sub(1);
            }
            run.protection = tuning.protection_seconds;
            if run.lives == 0 {
                run.finished = true;
                run.boosting = false;
                run.speed = 0.0;
            }
            break;
        }
    }
}
