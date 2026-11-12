//! Garage workflow. Confirmed snapshots drive both the UI and the next drive.
use crate::player::{Controls, PlayerTuning};
use crate::profile::{Account, ProfilePhase, Storage};
use crate::run::Run;
use crate::simulation::GameSystems;
use crate::storage::{StoreError, StoreResult};
use crate::vehicles::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, block_on, poll_once};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Owned,
    Catalog,
    Market,
}
#[derive(Clone, Debug)]
pub enum Intent {
    Tab(Tab),
    Item(usize),
    Action(GarageAction),
    Confirm,
    Cancel,
    Start,
    Open,
    Retry,
    Refresh(u32),
}

#[derive(Resource)]
pub struct Garage {
    pub open: bool,
    pub tab: Tab,
    pub selected: usize,
    pub data: Option<GarageSnapshot>,
    pub confirmation: Option<(GarageRequest, String)>,
    pub problem: Option<String>,
    pub notice: String,
    pub revision: u64,
    pub focus: usize,
    pub intents: Vec<Intent>,
    pending: Option<GarageRequest>,
    job: Option<Task<StoreResult<GarageSnapshot>>>,
    initial_attempted: bool,
    beginning: Option<Task<StoreResult<Option<crate::drive_proof::DriveTicket>>>>,
    begin_request: Option<String>,
}
impl Default for Garage {
    fn default() -> Self {
        Self {
            open: true,
            tab: Tab::Owned,
            selected: 0,
            data: None,
            confirmation: None,
            problem: None,
            notice: String::new(),
            revision: 0,
            focus: 0,
            intents: Vec::new(),
            pending: None,
            job: None,
            initial_attempted: false,
            beginning: None,
            begin_request: None,
        }
    }
}
impl Garage {
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.beginning.is_some()
    }
    pub fn selected_car(&self) -> Option<Vehicle> {
        match self.tab {
            Tab::Catalog => MODELS
                .get(self.selected)
                .map(|m| Vehicle::new("preview".into(), *m)),
            Tab::Owned => self.data.as_ref()?.vehicles.get(self.selected).cloned(),
            Tab::Market => self.data.as_ref()?.listings.get(self.selected).cloned(),
        }
    }
    pub fn enqueue(&mut self, intent: Intent) {
        self.intents.push(intent);
    }
}

pub struct GaragePlugin;
impl Plugin for GaragePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Garage>()
            .init_resource::<ActiveVehicle>()
            .configure_sets(FixedUpdate, GameSystems.run_if(garage_closed))
            .add_systems(Update, update_garage);
    }
}
pub fn garage_closed(garage: Res<Garage>) -> bool {
    !garage.open
}

fn quote(data: &GarageSnapshot, action: &GarageAction) -> Result<String, String> {
    let owned = |id: &String| {
        data.vehicles
            .iter()
            .find(|c| c.id == *id)
            .ok_or("Vehicle not owned".to_owned())
    };
    let (cost, description) = match action {
        GarageAction::Buy(model) => (model.price(), format!("Buy {}", model.name())),
        GarageAction::BuyListing(id) => {
            let car = data
                .listings
                .iter()
                .find(|c| c.id == *id)
                .ok_or("Listing unavailable")?;
            (
                car.market_price(),
                format!("Buy used {} with all upgrades", car.model.name()),
            )
        }
        GarageAction::Sell(id) => {
            let car = owned(id)?;
            if data.vehicles.len() <= 1 {
                return Err("Keep at least one vehicle".into());
            }
            if data.active == *id {
                return Err("Select another vehicle before selling".into());
            }
            return Ok(format!(
                "Sell {} now?\nReceive {} coins immediately.\nThe marketplace becomes the owner.\nBalance after sale: {}",
                car.model.name(),
                car.sale_price(),
                data.coins + car.sale_price()
            ));
        }
        GarageAction::Configure { vehicle, change } => {
            let car = owned(vehicle)?;
            let mut preview = car.clone();
            let cost = preview.configure(change)?;
            let t = preview.tuning();
            (
                cost,
                format!(
                    "Update {}\n{} / {} tires / motor {} / carbon {}\nCruise {:.0} | Boost {:.0} | Steering {:.0}",
                    car.model.name(),
                    preview.paint.name(),
                    preview.tires.name(),
                    preview.motor,
                    preview.carbon,
                    t.cruise_speed,
                    t.boost_speed,
                    t.steer_speed
                ),
            )
        }
        GarageAction::Select(id) => (0, format!("Drive {}", owned(id)?.model.name())),
        GarageAction::Refresh { .. } => return Err("Refresh has no quote".into()),
    };
    let remaining = data
        .coins
        .checked_sub(cost)
        .ok_or_else(|| format!("Need {cost} coins; balance {}", data.coins))?;
    Ok(format!(
        "{description}\nCost: {cost} coins | Balance after: {remaining}"
    ))
}

fn start_job(garage: &mut Garage, storage: &Storage, account: &Account, request: GarageRequest) {
    let Some(profile) = &account.profile else {
        return;
    };
    let future = storage.0.garage(profile.id.clone(), request.clone());
    garage.pending = Some(request);
    garage.job = Some(IoTaskPool::get().spawn(future));
    garage.problem = None;
    garage.revision += 1;
}

#[derive(SystemParam)]
struct DriveState<'w> {
    run: ResMut<'w, Run>,
    controls: ResMut<'w, Controls>,
    active: ResMut<'w, ActiveVehicle>,
    tuning: ResMut<'w, PlayerTuning>,
    recording: ResMut<'w, crate::drive_proof::Recording>,
    next_phase: ResMut<'w, NextState<ProfilePhase>>,
}

fn update_garage(
    mut garage: ResMut<Garage>,
    storage: Res<Storage>,
    mut account: ResMut<Account>,
    phase: Res<State<ProfilePhase>>,
    drive: DriveState,
) {
    let DriveState {
        mut run,
        mut controls,
        mut active,
        mut tuning,
        mut recording,
        mut next_phase,
    } = drive;
    if *phase.get() != ProfilePhase::Playing {
        return;
    }
    if let Some(result) = garage
        .beginning
        .as_mut()
        .and_then(|task| block_on(poll_once(task)))
    {
        garage.beginning = None;
        match result {
            Ok(ticket) => {
                if let Some(ticket) = &ticket {
                    *tuning = ticket.vehicle.tuning();
                    active.0 = ticket.vehicle.clone();
                }
                run.reported = true; // The old drive was already credited; this reset starts a new ticket.
                recording.ticket = ticket;
                recording.prepare = true;
                garage.begin_request = None;
                garage.open = false;
                controls.restart = true;
                garage.problem = None;
            }
            Err(error) => {
                garage.problem = Some(error.to_string());
            }
        }
        garage.revision += 1;
    }
    if let Some(result) = garage
        .job
        .as_mut()
        .and_then(|task| block_on(poll_once(task)))
    {
        garage.job = None;
        match result {
            Ok(data) => {
                if let Some(profile) = &mut account.profile {
                    profile.coins = data.coins;
                }
                if let Some(car) = data.vehicles.iter().find(|c| c.id == data.active) {
                    *tuning = car.tuning();
                    active.0 = car.clone();
                }
                garage.data = Some(data);
                garage.pending = None;
                garage.problem = None;
                garage.notice = "Saved. Your garage and balance are up to date.".into();
                garage.confirmation = None;
                let len = match garage.tab {
                    Tab::Catalog => MODELS.len(),
                    Tab::Owned => garage.data.as_ref().map_or(0, |d| d.vehicles.len()),
                    Tab::Market => garage.data.as_ref().map_or(0, |d| d.listings.len()),
                };
                garage.selected = garage.selected.min(len.saturating_sub(1));
            }
            Err(error) => {
                // A transport error may have committed. Only retry the SAME request
                // until its outcome is known; never create a second transaction ID.
                if !matches!(error, StoreError::Unavailable(_)) {
                    garage.pending = None;
                }
                garage.problem = Some(error.to_string());
                garage.confirmation = None;
            }
        }
        garage.revision += 1;
    }
    if !garage.initial_attempted && account.profile.is_some() && !account.saving() {
        garage.initial_attempted = true;
        start_job(
            &mut garage,
            &storage,
            &account,
            GarageRequest::new(GarageAction::Refresh { page: 0 }),
        );
    }
    let intents = std::mem::take(&mut garage.intents);
    for intent in intents {
        if garage.busy() {
            continue;
        }
        if account.saving() {
            if matches!(intent, Intent::Open) && run.finished {
                garage.open = true;
                *controls = Controls::default();
            }
            if matches!(intent, Intent::Retry) {
                account.retry();
            }
            garage.problem = Some("Waiting for drive coins to be saved".into());
            garage.revision += 1;
            continue;
        }
        if (garage.pending.is_some() || garage.begin_request.is_some())
            && !matches!(intent, Intent::Retry)
        {
            continue;
        }
        match intent {
            Intent::Open if run.finished || run.distance == 0. => {
                garage.open = true;
                *controls = Controls::default();
                if account.profile.is_some() {
                    start_job(
                        &mut garage,
                        &storage,
                        &account,
                        GarageRequest::new(GarageAction::Refresh { page: 0 }),
                    );
                }
            }
            Intent::Start if garage.open && garage.confirmation.is_none() => {
                if garage.data.is_some() || (account.profile.is_none() && !storage.0.online()) {
                    let id = crate::storage::RunId::random().0;
                    garage.begin_request = Some(id.clone());
                    garage.beginning = Some(IoTaskPool::get().spawn(storage.0.begin_drive(id)));
                }
            }
            Intent::Tab(tab) => {
                garage.tab = tab;
                garage.selected = 0;
                garage.confirmation = None;
            }
            Intent::Item(item) => {
                garage.selected = item;
                garage.confirmation = None;
            }
            Intent::Cancel => garage.confirmation = None,
            Intent::Action(action) if garage.open => {
                if let Some(data) = &garage.data {
                    match quote(data, &action) {
                        Ok(message) => {
                            garage.problem = None;
                            garage.confirmation = Some((GarageRequest::new(action), message));
                            garage.focus = usize::MAX;
                        }
                        Err(error) => garage.problem = Some(error),
                    }
                }
            }
            Intent::Confirm if garage.open => {
                if let Some((request, _)) = garage.confirmation.take() {
                    start_job(&mut garage, &storage, &account, request);
                }
            }
            Intent::Retry => {
                if account.profile.is_none() {
                    garage.initial_attempted = false;
                    next_phase.set(ProfilePhase::Loading);
                    garage.revision += 1;
                    continue;
                }
                if let Some(id) = &garage.begin_request {
                    garage.beginning =
                        Some(IoTaskPool::get().spawn(storage.0.begin_drive(id.clone())));
                    garage.revision += 1;
                    continue;
                }
                let request = garage
                    .pending
                    .clone()
                    .unwrap_or_else(|| GarageRequest::new(GarageAction::Refresh { page: 0 }));
                start_job(&mut garage, &storage, &account, request);
            }
            Intent::Refresh(page) if garage.open => start_job(
                &mut garage,
                &storage,
                &account,
                GarageRequest::new(GarageAction::Refresh { page }),
            ),
            _ => {}
        }
        garage.revision += 1;
    }
}
