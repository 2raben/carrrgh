//! Car Drive: steer through traffic, manage boost, and drive as far as possible.

mod camera;
mod hud;
mod input;
mod level;
mod pickups;
mod player;
mod profile;
mod run;
mod simulation;
mod storage;
mod visuals;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use bevy::prelude::*;

use crate::storage::sqlite::SqliteStore;

fn main() -> AppExit {
    let path = SqliteStore::default_path();
    let location = path.as_ref().map(|path| path.display().to_string());
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
            profile::ProfilePlugin {
                store: Arc::new(SqliteStore::new(path)),
            },
            input::InputPlugin,
            visuals::VisualsPlugin,
            camera::CameraPlugin,
            hud::HudPlugin,
        ))
        .add_systems(Startup, move || match &location {
            Some(path) => info!("Saving profiles to {path}"),
            None => warn!("No data directory; coins cannot be saved"),
        })
        .run()
}
