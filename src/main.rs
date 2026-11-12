//! Car Drive: steer through traffic, manage boost, and drive as far as possible.

mod camera;
mod drive_proof;
mod garage;
mod garage_ui;
mod hud;
mod input;
mod level;
mod market;
mod pickups;
mod player;
mod profile;
mod run;
mod simulation;
mod storage;
mod vehicle_art;
mod vehicles;
mod visuals;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use bevy::prelude::*;

use crate::storage::sqlite::SqliteStore;

fn main() -> AppExit {
    if let Some(result) = market::command_line(&std::env::args().collect::<Vec<_>>()) {
        return match result {
            Ok(()) => AppExit::Success,
            Err(error) => {
                eprintln!("{error}");
                AppExit::error()
            }
        };
    }
    let path = SqliteStore::default_path();
    let location = path.as_ref().map(|path| path.display().to_string());
    let store: Arc<dyn storage::ProfileStore> =
        if let Some(config) = std::env::var_os("CAR_DRIVE_MARKET_CONFIG") {
            match market::NetworkStore::from_file(std::path::Path::new(&config)) {
                Ok(store) => Arc::new(store),
                Err(error) => {
                    eprintln!("Could not connect market account: {error}");
                    return AppExit::error();
                }
            }
        } else {
            Arc::new(SqliteStore::new(path))
        };
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
            profile::ProfilePlugin { store },
            garage::GaragePlugin,
            garage_ui::GarageUiPlugin,
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
