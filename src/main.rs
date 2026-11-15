//! Car Drive: steer through traffic, manage boost, and drive as far as possible.

mod bindings;
mod camera;
mod difficulty;
mod drive_proof;
mod garage;
mod garage_ui;
mod hud;
mod input;
mod level;
#[cfg(not(target_arch = "wasm32"))]
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

fn main() -> AppExit {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(result) = market::command_line(&std::env::args().collect::<Vec<_>>()) {
        return match result {
            Ok(()) => AppExit::Success,
            Err(error) => {
                eprintln!("{error}");
                AppExit::error()
            }
        };
    }
    let (store, location) = match profile_store() {
        Ok(found) => found,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(window()),
            ..default()
        }))
        .add_plugins((
            simulation::SimulationPlugin,
            profile::ProfilePlugin { store },
            garage::GaragePlugin,
            garage_ui::GarageUiPlugin,
            bindings::BindingsPlugin,
            input::InputPlugin,
            visuals::VisualsPlugin,
            camera::CameraPlugin,
            hud::HudPlugin,
        ))
        .add_systems(Startup, move || match &location {
            Some(place) => info!("Saving profiles to {place}"),
            None => warn!("No data directory; coins cannot be saved"),
        })
        .run()
}

/// A 960 × 800 window on the desktop.
#[cfg(not(target_arch = "wasm32"))]
fn window() -> Window {
    Window {
        title: "Car Drive".into(),
        resolution: (960, 800).into(),
        ..default()
    }
}

/// In the browser, the canvas of `web/index.html`, sized by the page. Bevy keeps
/// the browser's own handling of keys away from it, so the arrows, Space and
/// Tab drive and switch garage tabs instead of scrolling or leaving the page.
#[cfg(target_arch = "wasm32")]
fn window() -> Window {
    Window {
        title: "Car Drive".into(),
        canvas: Some("#car-drive".into()),
        fit_canvas_to_parent: true,
        ..default()
    }
}

type FoundStore = (Arc<dyn storage::ProfileStore>, Option<String>);

/// The private market account from `$CAR_DRIVE_MARKET_CONFIG`, otherwise the
/// local SQLite file (ADR 0007, ADR 0009).
#[cfg(not(target_arch = "wasm32"))]
fn profile_store() -> Result<FoundStore, String> {
    use crate::storage::sqlite::SqliteStore;
    let path = SqliteStore::default_path();
    let location = path.as_ref().map(|path| path.display().to_string());
    if let Some(config) = std::env::var_os("CAR_DRIVE_MARKET_CONFIG") {
        let store = market::NetworkStore::from_file(std::path::Path::new(&config))
            .map_err(|error| format!("Could not connect market account: {error}"))?;
        return Ok((Arc::new(store), location));
    }
    Ok((Arc::new(SqliteStore::new(path)), location))
}

/// The page's `localStorage` (ADR 0011). If the browser blocks it, every
/// operation reports storage as unavailable and the game says so.
#[cfg(target_arch = "wasm32")]
fn profile_store() -> Result<FoundStore, String> {
    let location = format!("local storage item `{}`", storage::browser::KEY);
    Ok((
        Arc::new(storage::browser::BrowserStore::default()),
        Some(location),
    ))
}
