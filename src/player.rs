//! Device-independent steering in a road-relative coordinate system.
use bevy::prelude::*;

use crate::level::{CAR_SIZE, PLAYER_START, ROAD_HALF_WIDTH};

#[derive(Component, Debug)]
pub struct Player;

/// Centre of a simulated body. Only presentation writes transforms.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Position(pub Vec2);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PreviousPosition(pub Vec2);

/// Held intent plus a restart edge latched until a fixed step consumes it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Controls {
    pub move_x: f32,
    pub boost: bool,
    pub brake: bool,
    pub restart: bool,
    pub end_drive: bool,
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct PlayerTuning {
    pub size: Vec2,
    pub steer_speed: f32,
    pub cruise_speed: f32,
    pub boost_speed: f32,
    /// Fraction of the full tank per second.
    pub boost_drain: f32,
    pub boost_recharge: f32,
    pub protection_seconds: f32,
}

impl PlayerTuning {
    /// Play-test value; every catalogue model still overtakes 140-unit/s traffic.
    pub fn brake_speed(&self) -> f32 {
        self.cruise_speed * 0.75
    }
}

impl Default for PlayerTuning {
    fn default() -> Self {
        Self {
            size: CAR_SIZE,
            steer_speed: 420.0,
            cruise_speed: 300.0,
            boost_speed: 540.0,
            boost_drain: 0.4,
            boost_recharge: 0.2,
            protection_seconds: 1.4,
        }
    }
}

pub fn spawn_player(mut commands: Commands) {
    commands.spawn((
        Name::new("Player"),
        Player,
        Position(PLAYER_START),
        PreviousPosition(PLAYER_START),
    ));
}

pub fn step_player(
    time: Res<Time>,
    tuning: Res<PlayerTuning>,
    controls: Res<Controls>,
    mut players: Query<(&mut Position, &mut PreviousPosition), With<Player>>,
) {
    for (mut position, mut previous) in &mut players {
        previous.0 = position.0;
        let limit = ROAD_HALF_WIDTH - tuning.size.x * 0.5;
        position.0.x = (position.0.x
            + controls.move_x.clamp(-1.0, 1.0) * tuning.steer_speed * time.delta_secs())
        .clamp(-limit, limit);
    }
}
