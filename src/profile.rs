//! The local player profile: load or create it before the first drive, then
//! credit each drive's coins through a [`ProfileStore`].
//!
//! Storage runs asynchronously; no system waits for it except on exit. Without
//! a confirmed profile the simulation does not start. If loading fails, the game
//! stays playable without saving and says so.
use std::collections::VecDeque;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::tasks::{BoxedFuture, IoTaskPool, Task, block_on, poll_once};

use crate::run::{Run, RunEnded};
use crate::simulation::GameSystems;
use crate::storage::{PlayerName, Profile, ProfileStore, RunCredit, RunId, StoreResult};

pub const DEFAULT_NAME: &str = "Driver";

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProfilePhase {
    #[default]
    Loading,
    /// No profile exists yet; the player chooses a name.
    Creating,
    Playing,
}

#[derive(Resource, Clone)]
pub struct Storage(pub Arc<dyn ProfileStore>);

/// The confirmed profile and the credits still waiting for storage.
#[derive(Resource, Debug, Default)]
pub struct Account {
    /// As last confirmed by storage; `None` while loading or playing without storage.
    pub profile: Option<Profile>,
    /// The latest storage failure, shown until an operation succeeds.
    pub problem: Option<String>,
    pending: VecDeque<RunCredit>,
    /// After a failed credit, wait for the next drive's end before retrying.
    paused: bool,
}

impl Account {
    /// Whether credited coins are still waiting for confirmation.
    pub fn saving(&self) -> bool {
        !self.pending.is_empty()
    }

    fn enqueue(&mut self, coins: u64) {
        let Some(profile) = &self.profile else {
            return;
        };
        if coins > 0 {
            self.pending.push_back(RunCredit {
                run: RunId::random(),
                profile: profile.id.clone(),
                coins,
            });
        }
        self.paused = false;
    }
}

/// The name typed on the create-player screen.
#[derive(Resource, Debug)]
pub struct NameDraft {
    pub text: String,
    /// Set by input; consumed when the profile is requested.
    pub submitted: bool,
    pub saving: bool,
    pub error: Option<String>,
}

impl Default for NameDraft {
    fn default() -> Self {
        Self {
            text: DEFAULT_NAME.into(),
            submitted: false,
            saving: false,
            error: None,
        }
    }
}

enum Outcome {
    Loaded(StoreResult<Option<Profile>>),
    Created(StoreResult<Profile>),
    Credited(StoreResult<Profile>),
}

/// At most one storage operation at a time, so credits apply in order.
#[derive(Resource, Default)]
struct StorageJob(Option<Job>);

enum Job {
    Finished(Outcome),
    Running(Task<Outcome>),
}

impl StorageJob {
    fn busy(&self) -> bool {
        self.0.is_some()
    }

    /// Stores that answer at once finish here; others run on the I/O pool.
    fn start(&mut self, mut future: BoxedFuture<'static, Outcome>) {
        self.0 = Some(match block_on(poll_once(&mut future)) {
            Some(outcome) => Job::Finished(outcome),
            None => Job::Running(IoTaskPool::get().spawn(future)),
        });
    }

    fn take_finished(&mut self) -> Option<Outcome> {
        match self.0.take()? {
            Job::Finished(outcome) => Some(outcome),
            Job::Running(mut task) => {
                let outcome = block_on(poll_once(&mut task));
                if outcome.is_none() {
                    self.0 = Some(Job::Running(task));
                }
                outcome
            }
        }
    }

    fn wait(&mut self) -> Option<Outcome> {
        match self.0.take()? {
            Job::Finished(outcome) => Some(outcome),
            Job::Running(task) => Some(block_on(task)),
        }
    }
}

pub struct ProfilePlugin {
    pub store: Arc<dyn ProfileStore>,
}

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.insert_resource(Storage(self.store.clone()))
            .init_resource::<Account>()
            .init_resource::<NameDraft>()
            .init_resource::<StorageJob>()
            .init_state::<ProfilePhase>()
            .configure_sets(
                FixedUpdate,
                GameSystems.run_if(in_state(ProfilePhase::Playing)),
            )
            .add_systems(OnEnter(ProfilePhase::Loading), load_profile)
            .add_systems(FixedUpdate, queue_credits.after(GameSystems))
            .add_systems(
                Update,
                (
                    create_profile.run_if(in_state(ProfilePhase::Creating)),
                    send_credit,
                    finish_job,
                )
                    .chain(),
            )
            .add_systems(Last, flush_on_exit);
    }
}

fn load_profile(storage: Res<Storage>, mut job: ResMut<StorageJob>) {
    let load = storage.0.load();
    job.start(Box::pin(async move { Outcome::Loaded(load.await) }));
}

fn create_profile(
    storage: Res<Storage>,
    mut draft: ResMut<NameDraft>,
    mut job: ResMut<StorageJob>,
) {
    if !std::mem::take(&mut draft.submitted) || job.busy() {
        return;
    }
    match PlayerName::parse(&draft.text) {
        Ok(name) => {
            draft.error = None;
            draft.saving = true;
            let create = storage.0.create(name);
            job.start(Box::pin(async move { Outcome::Created(create.await) }));
        }
        Err(error) => draft.error = Some(error.to_string()),
    }
}

fn queue_credits(mut ended: MessageReader<RunEnded>, mut account: ResMut<Account>) {
    for run in ended.read() {
        account.enqueue(run.coins);
    }
}

fn send_credit(storage: Res<Storage>, account: Res<Account>, mut job: ResMut<StorageJob>) {
    if job.busy() || account.paused {
        return;
    }
    if let Some(credit) = account.pending.front() {
        let credit = storage.0.credit(credit.clone());
        job.start(Box::pin(async move { Outcome::Credited(credit.await) }));
    }
}

fn finish_job(
    mut job: ResMut<StorageJob>,
    mut account: ResMut<Account>,
    mut draft: ResMut<NameDraft>,
    mut phase: ResMut<NextState<ProfilePhase>>,
) {
    if let Some(outcome) = job.take_finished() {
        apply(outcome, &mut account, &mut draft, &mut phase);
    }
}

fn apply(
    outcome: Outcome,
    account: &mut Account,
    draft: &mut NameDraft,
    phase: &mut NextState<ProfilePhase>,
) {
    match outcome {
        Outcome::Loaded(Ok(Some(profile))) | Outcome::Created(Ok(profile)) => {
            info!("Driving as {}", profile.name.as_str());
            account.profile = Some(profile);
            account.problem = None;
            draft.saving = false;
            phase.set(ProfilePhase::Playing);
        }
        Outcome::Loaded(Ok(None)) => phase.set(ProfilePhase::Creating),
        Outcome::Loaded(Err(error)) => {
            warn!("Playing without saving: {error}");
            account.problem = Some(format!("Coins are not saved: {error}"));
            phase.set(ProfilePhase::Playing);
        }
        Outcome::Created(Err(error)) => {
            warn!("Could not create profile: {error}");
            draft.saving = false;
            draft.error = Some(format!("Could not save: {error}"));
        }
        Outcome::Credited(Ok(profile)) => {
            account.pending.pop_front();
            account.profile = Some(profile);
            account.problem = None;
        }
        Outcome::Credited(Err(error)) => {
            warn!("Could not credit coins: {error}");
            account.problem = Some(format!("Coins not saved yet: {error}"));
            account.paused = true;
        }
    }
}

/// Quitting ends the current drive. Credits are written before the app exits,
/// since a background task would not outlive it.
fn flush_on_exit(
    mut exits: MessageReader<AppExit>,
    storage: Res<Storage>,
    mut run: ResMut<Run>,
    mut account: ResMut<Account>,
    mut job: ResMut<StorageJob>,
    mut draft: ResMut<NameDraft>,
    mut phase: ResMut<NextState<ProfilePhase>>,
) {
    if exits.read().count() == 0 {
        return;
    }
    if let Some(outcome) = job.wait() {
        apply(outcome, &mut account, &mut draft, &mut phase);
    }
    if !std::mem::replace(&mut run.reported, true) {
        account.enqueue(run.coins());
    }
    while let Some(credit) = account.pending.front().cloned() {
        let outcome = Outcome::Credited(block_on(storage.0.credit(credit)));
        apply(outcome, &mut account, &mut draft, &mut phase);
        if account.paused {
            break;
        }
    }
}
