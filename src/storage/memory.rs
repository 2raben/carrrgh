//! An in-memory store for headless tests; every operation completes immediately.
use std::collections::HashSet;
use std::future::ready;
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::tasks::BoxedFuture;

use super::{PlayerName, Profile, ProfileId, ProfileStore, RunCredit, RunId, StoreError};

#[derive(Clone, Default)]
pub struct MemoryStore(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    profiles: Vec<Profile>,
    credited: HashSet<RunId>,
    failure: Option<StoreError>,
}

impl MemoryStore {
    pub fn with_profile(name: &str, coins: u64) -> Self {
        let store = Self::default();
        store.state().profiles.push(Profile {
            id: ProfileId("p0".into()),
            name: PlayerName::parse(name).unwrap(),
            coins,
        });
        store
    }

    /// Makes every following operation fail until cleared.
    pub fn fail_with(&self, failure: Option<StoreError>) {
        self.state().failure = failure;
    }

    pub fn profiles(&self) -> Vec<Profile> {
        self.state().profiles.clone()
    }

    pub fn coins(&self) -> u64 {
        self.state().profiles.first().map_or(0, |p| p.coins)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    fn answer<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut State) -> Result<T, StoreError>,
    ) -> BoxedFuture<'static, Result<T, StoreError>> {
        let mut state = self.state();
        let result = match state.failure.clone() {
            Some(failure) => Err(failure),
            None => work(&mut state),
        };
        Box::pin(ready(result))
    }
}

impl ProfileStore for MemoryStore {
    fn load(&self) -> BoxedFuture<'static, Result<Option<Profile>, StoreError>> {
        self.answer(|state| Ok(state.profiles.first().cloned()))
    }

    fn create(&self, name: PlayerName) -> BoxedFuture<'static, Result<Profile, StoreError>> {
        self.answer(|state| {
            let profile = Profile {
                id: ProfileId(format!("p{}", state.profiles.len())),
                name,
                coins: 0,
            };
            state.profiles.push(profile.clone());
            Ok(profile)
        })
    }

    fn credit(&self, credit: RunCredit) -> BoxedFuture<'static, Result<Profile, StoreError>> {
        self.answer(|state| {
            let profile = state
                .profiles
                .iter_mut()
                .find(|p| p.id == credit.profile)
                .ok_or(StoreError::UnknownProfile)?;
            if state.credited.insert(credit.run) {
                profile.coins += credit.coins;
            }
            Ok(profile.clone())
        })
    }
}
