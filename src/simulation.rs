//! Fixed-step simulation, usable with MinimalPlugins and no window.
use crate::{level, pickups, player, run};
use bevy::prelude::*;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameSystems;

pub struct SimulationPlugin;
impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<player::PlayerTuning>()
            .init_resource::<crate::drive_proof::Recording>()
            .init_resource::<player::Controls>()
            .init_resource::<run::Run>()
            .init_resource::<level::TrafficFlow>()
            .init_resource::<pickups::PickupFlow>()
            .add_message::<run::RunEnded>()
            .add_systems(Startup, player::spawn_player)
            .add_systems(
                FixedUpdate,
                (
                    run::report_abandoned_run,
                    run::restart_run,
                    crate::drive_proof::prepare_recording,
                    (
                        crate::drive_proof::record_controls,
                        run::advance_run,
                        player::step_player,
                        level::spawn_traffic,
                        pickups::spawn_pickups,
                        level::plan_lane_changes,
                        level::move_traffic,
                        pickups::move_pickups,
                        pickups::collect_pickups,
                        run::collide,
                        level::remove_offscreen,
                        crate::drive_proof::limit_online_drive,
                    )
                        .chain()
                        .run_if(run::driving),
                    run::report_finished_run,
                )
                    .chain()
                    .in_set(GameSystems),
            );
    }
}
