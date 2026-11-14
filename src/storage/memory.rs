//! An in-memory store for headless tests; every operation completes immediately.
//!
//! It keeps the encoded save document text, like the browser keeps it in
//! `localStorage`, so tests exercise the same open, apply and write path.
use std::sync::{Arc, Mutex, MutexGuard};

use super::document::{self, DocumentStore, SaveDocument, Slot};
use super::{PlayerName, Profile, RunCredit, RunId, StoreError, StoreResult};

pub type MemoryStore = DocumentStore<MemorySlot>;

#[derive(Clone, Default)]
pub struct MemorySlot(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    text: Option<String>,
    failure: Option<StoreError>,
}

impl MemorySlot {
    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }
}

impl Slot for MemorySlot {
    fn update<T>(&self, work: impl FnOnce(&mut SaveDocument) -> StoreResult<T>) -> StoreResult<T> {
        let mut state = self.state();
        if let Some(failure) = state.failure.clone() {
            return Err(failure);
        }
        let (value, text) = document::apply(document::open(state.text.as_deref())?, work)?;
        if text.is_some() {
            state.text = text;
        }
        Ok(value)
    }
}

impl MemoryStore {
    pub fn with_profile(name: &str, coins: u64) -> Self {
        let store = Self::default();
        store
            .0
            .update(|d| {
                let profile = d.create(PlayerName::parse(name).unwrap());
                d.credit(&RunCredit {
                    run: RunId::random(),
                    profile: profile.id,
                    coins,
                    proof: None,
                })
            })
            .unwrap();
        store
    }

    /// Makes every following operation fail until cleared.
    pub fn fail_with(&self, failure: Option<StoreError>) {
        self.0.state().failure = failure;
    }

    pub fn profiles(&self) -> Vec<Profile> {
        document::open(self.0.state().text.as_deref())
            .unwrap()
            .profiles()
    }

    pub fn coins(&self) -> u64 {
        self.profiles().first().map_or(0, |p| p.coins)
    }
}
