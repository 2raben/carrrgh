//! Rare hearts and shields lying on the road; no rendering or device input.
use bevy::prelude::*;

use crate::level::{CAR_SIZE, LANES, PLAYER_START, SPAWN_Y, Traffic};
use crate::player::{Player, PlayerTuning, Position, PreviousPosition};
use crate::run::{MAX_LIVES, Run, swept_hit};

pub const PICKUP_SIZE: Vec2 = Vec2::splat(36.0);
/// Minimum distance between a pickup and any car in its lane until it passes the player.
pub const PICKUP_CLEARANCE: f32 = 240.0;
/// Below this line a pickup has passed the player and no longer needs clearance.
#[cfg(test)]
pub const PICKUP_PASSED_Y: f32 = PLAYER_START.y - CAR_SIZE.y;
/// Road distance between pickups, in world units (ten per metre).
pub const PICKUP_INTERVAL: std::ops::Range<f32> = 4_000.0..9_000.0;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pickup {
    /// Restores one lost heart.
    Heart,
    /// Held until the next collision, which then costs no heart.
    Shield,
}

#[derive(Resource)]
pub struct PickupFlow {
    /// Road distance until the next pickup is due.
    pub remaining: f32,
    pub rng: fastrand::Rng,
}

impl PickupFlow {
    pub fn with_seed(seed: u64) -> Self {
        Self::from_rng(fastrand::Rng::with_seed(seed))
    }

    fn from_rng(mut rng: fastrand::Rng) -> Self {
        Self {
            remaining: Self::interval(&mut rng),
            rng,
        }
    }

    fn interval(rng: &mut fastrand::Rng) -> f32 {
        PICKUP_INTERVAL.start + rng.f32() * (PICKUP_INTERVAL.end - PICKUP_INTERVAL.start)
    }
}

impl Default for PickupFlow {
    fn default() -> Self {
        Self::from_rng(fastrand::Rng::new())
    }
}

/// Whether a car in a pickup's lane stays at least `PICKUP_CLEARANCE` away
/// until the pickup passes the player. Their gap changes only by the car's own
/// speed; the initial cruise speed gives the pickup its longest lifetime.
#[cfg(test)]
pub fn keeps_clear(pickup_y: f32, car_y: f32, car_speed: f32, cruise_speed: f32) -> bool {
    keeps_clear_for(
        pickup_y,
        car_y,
        car_speed,
        &PlayerTuning {
            cruise_speed,
            ..default()
        },
    )
}
pub fn keeps_clear_for(pickup_y: f32, car_y: f32, car_speed: f32, tuning: &PlayerTuning) -> bool {
    let passed_y = PLAYER_START.y - tuning.size.y.max(CAR_SIZE.y);
    let lifetime = ((pickup_y - passed_y) / tuning.cruise_speed).max(0.0);
    let clearance = PICKUP_CLEARANCE + (tuning.size.y - CAR_SIZE.y).max(0.0);
    let start = car_y - pickup_y;
    let end = start + car_speed * lifetime;
    // The gap is linear in time, so clear endpoints on one side clear the whole span.
    start.signum() == end.signum() && start.abs().min(end.abs()) >= clearance
}

pub fn spawn_pickups(
    mut commands: Commands,
    run: Res<Run>,
    tuning: Res<PlayerTuning>,
    mut flow: ResMut<PickupFlow>,
    pickups: Query<(), With<Pickup>>,
    traffic: Query<(&Traffic, &Position)>,
) {
    flow.remaining -= (run.distance - run.previous_distance) as f32;
    if flow.remaining > 0.0 || !pickups.is_empty() {
        return;
    }
    let allowed: Vec<_> = [
        (run.lives < MAX_LIVES).then_some(Pickup::Heart),
        (!run.shield).then_some(Pickup::Shield),
    ]
    .into_iter()
    .flatten()
    .collect();
    let Some(&kind) = flow.rng.choice(&allowed) else {
        // Nothing would help now: skip this slot instead of rewarding the next hit.
        flow.remaining = PickupFlow::interval(&mut flow.rng);
        return;
    };
    let mut lanes = [0, 1, 2];
    flow.rng.shuffle(&mut lanes);
    // If no lane is clear, the pickup stays due and is retried next tick.
    let Some(lane) = lanes.into_iter().find(|&lane| {
        traffic
            .iter()
            .filter(|(car, _)| car.occupies(lane))
            .all(|(car, p)| keeps_clear_for(SPAWN_Y, p.0.y, car.speed, &tuning))
    }) else {
        return;
    };
    let at = Vec2::new(LANES[lane], SPAWN_Y);
    commands.spawn((
        Name::new("Pickup"),
        kind,
        Position(at),
        PreviousPosition(at),
    ));
    flow.remaining = PickupFlow::interval(&mut flow.rng);
}

/// Pickups lie still on the road, so they move at the player's speed.
pub fn move_pickups(
    time: Res<Time>,
    run: Res<Run>,
    mut pickups: Query<(&mut Position, &mut PreviousPosition), With<Pickup>>,
) {
    for (mut position, mut previous) in &mut pickups {
        previous.0 = position.0;
        position.0.y -= run.speed * time.delta_secs();
    }
}

/// Collecting uses the same swept test as traffic, so boost cannot skip pickups.
/// A pickup that would have no effect stays on the road.
pub fn collect_pickups(
    mut commands: Commands,
    tuning: Res<PlayerTuning>,
    mut run: ResMut<Run>,
    players: Query<(&Position, &PreviousPosition), With<Player>>,
    pickups: Query<(Entity, &Pickup, &Position, &PreviousPosition)>,
) {
    let Ok((player, previous_player)) = players.single() else {
        return;
    };
    for (entity, pickup, position, previous) in &pickups {
        if !swept_hit(
            previous.0 - previous_player.0,
            position.0 - player.0,
            (tuning.size + PICKUP_SIZE) * 0.5,
        ) {
            continue;
        }
        match pickup {
            Pickup::Heart if run.lives < MAX_LIVES => run.lives += 1,
            Pickup::Shield if !run.shield => run.shield = true,
            _ => continue,
        }
        commands.entity(entity).despawn();
    }
}
