//! Fixed-step input recording and server verification using the real simulation.
use crate::level::TrafficFlow;
use crate::pickups::PickupFlow;
use crate::player::Controls;
use crate::run::Run;
use crate::vehicles::Vehicle;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use serde::{Deserialize, Serialize};

pub const MAX_DRIVE_TICKS: usize = 64 * 60 * 20;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriveTicket {
    pub id: String,
    pub seed: u64,
    pub vehicle: Vehicle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame(pub i8, pub bool);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveProof {
    pub ticket: String,
    pub frames: Vec<Frame>,
}

#[derive(Resource, Default)]
pub struct Recording {
    pub ticket: Option<DriveTicket>,
    pub frames: Vec<Frame>,
    pub prepare: bool,
}
impl Recording {
    pub fn proof(&self) -> Option<DriveProof> {
        self.ticket.as_ref().map(|ticket| DriveProof {
            ticket: ticket.id.clone(),
            frames: self.frames.clone(),
        })
    }
}
pub fn prepare_recording(
    mut recording: ResMut<Recording>,
    mut traffic: ResMut<TrafficFlow>,
    mut pickups: ResMut<PickupFlow>,
) {
    if !recording.prepare {
        return;
    }
    if let Some(ticket) = &recording.ticket {
        *traffic = TrafficFlow {
            cooldown: 0.,
            rng: fastrand::Rng::with_seed(ticket.seed),
            ..default()
        };
        *pickups = PickupFlow::with_seed(ticket.seed ^ 0x7472_6166_6669_6301);
    }
    recording.frames.clear();
    recording.prepare = false;
}
pub fn record_controls(mut controls: ResMut<Controls>, mut recording: ResMut<Recording>) {
    if recording.ticket.is_none() {
        return;
    }
    let steer = (controls.move_x.clamp(-1., 1.) * 127.).round() as i8;
    controls.move_x = f32::from(steer) / 127.;
    recording.frames.push(Frame(steer, controls.boost));
}
pub fn limit_online_drive(recording: Res<Recording>, mut run: ResMut<Run>) {
    if recording.ticket.is_some() && recording.frames.len() >= MAX_DRIVE_TICKS {
        run.finished = true;
        run.boosting = false;
        run.speed = 0.;
    }
}

/// No client-supplied score is used. Reject frames beyond game over and replay
/// only bounded, quantized Controls on the same fixed-step simulation.
pub fn verify(ticket: &DriveTicket, proof: &DriveProof) -> Result<u64, String> {
    if proof.ticket != ticket.id || proof.frames.len() > MAX_DRIVE_TICKS {
        return Err("Invalid drive proof".into());
    }
    ticket.vehicle.validate()?;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, crate::simulation::SimulationPlugin))
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
        .insert_resource(ticket.vehicle.tuning())
        .insert_resource(TrafficFlow {
            cooldown: 0.,
            rng: fastrand::Rng::with_seed(ticket.seed),
            ..default()
        })
        .insert_resource(PickupFlow::with_seed(ticket.seed ^ 0x7472_6166_6669_6301));
    app.finish();
    app.cleanup();
    app.update();
    for frame in &proof.frames {
        if frame.0 == i8::MIN || app.world().resource::<Run>().finished {
            return Err("Invalid input or drive continued after game over".into());
        }
        *app.world_mut().resource_mut::<Controls>() = Controls {
            move_x: f32::from(frame.0) / 127.,
            boost: frame.1,
            restart: false,
            end_drive: false,
        };
        app.update();
    }
    Ok(app.world().resource::<Run>().coins())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicles::{Change, MODELS, Upgrade};

    #[test]
    fn recorded_drives_replay_exactly_for_every_vehicle_and_tuning() {
        for model in MODELS {
            let mut car = Vehicle::new("test".into(), model);
            for upgrade in [
                Upgrade::Motor1,
                Upgrade::Motor2,
                Upgrade::SportTires,
                Upgrade::Carbon,
            ] {
                car.configure(&Change::Buy(upgrade)).unwrap();
            }
            let ticket = DriveTicket {
                id: "drive".into(),
                seed: 814,
                vehicle: car,
            };
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, crate::simulation::SimulationPlugin))
                .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
                .insert_resource(ticket.vehicle.tuning())
                .insert_resource(Recording {
                    ticket: Some(ticket.clone()),
                    prepare: true,
                    ..default()
                });
            app.finish();
            app.cleanup();
            app.update();
            for tick in 0..3000 {
                if app.world().resource::<Run>().finished {
                    break;
                }
                let mut controls = app.world_mut().resource_mut::<Controls>();
                controls.move_x = if tick % 240 < 120 { -0.673 } else { 0.371 };
                controls.boost = tick % 320 < 160;
                app.update();
            }
            let proof = app.world().resource::<Recording>().proof().unwrap();
            let run = app.world().resource::<Run>();
            assert!(!proof.frames.is_empty());
            assert_eq!(
                verify(&ticket, &proof).unwrap(),
                run.coins(),
                "{}",
                model.name()
            );
            if run.finished {
                let mut too_long = proof.clone();
                too_long.frames.push(Frame(0, false));
                assert!(verify(&ticket, &too_long).is_err());
            }
        }
    }
}
