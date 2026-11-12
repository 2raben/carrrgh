//! Car Drive: steer through traffic, manage boost, and drive as far as possible.

mod camera;
mod hud;
mod input;
mod level;
mod player;
mod run;
mod simulation;
mod visuals;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Car Drive".into(),
                resolution: (960, 800).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            simulation::SimulationPlugin,
            input::InputPlugin,
            visuals::VisualsPlugin,
            camera::CameraPlugin,
            hud::HudPlugin,
        ))
        .run()
}
