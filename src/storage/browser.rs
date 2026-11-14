//! The browser store: the whole save as one `localStorage` item,
//! `car-drive/save` (ADR 0011).
//!
//! Each operation reads the item, applies the change to the save document and
//! writes the item back with one `setItem` before its future is returned. No
//! browser object is kept between calls: the store must be shareable between
//! threads.
//!
//! All pages of an origin share its `localStorage`; on GitHub Pages that is
//! every project page of the account. The prefix avoids accidental collisions,
//! and the text is decoded as untrusted input.
use bevy::log::warn;

use super::document::{self, DocumentStore, SaveDocument, Slot};
use super::{StoreError, StoreResult};

pub const KEY: &str = "car-drive/save";
/// Where an unusable save is moved, so a new one can start without losing it.
const BROKEN_KEY: &str = "car-drive/save.broken";

pub type BrowserStore = DocumentStore<LocalStorage>;

#[derive(Default)]
pub struct LocalStorage;

fn unavailable(error: impl std::fmt::Debug) -> StoreError {
    StoreError::Unavailable(format!("{error:?}"))
}

/// Fetched for every call; `None` from the browser means storage is blocked.
fn storage() -> StoreResult<web_sys::Storage> {
    web_sys::window()
        .ok_or_else(|| StoreError::Unavailable("no window".into()))?
        .local_storage()
        .map_err(unavailable)?
        .ok_or_else(|| StoreError::Unavailable("local storage is blocked".into()))
}

/// The stored document. An invalid one is moved aside and replaced by an empty
/// save; a newer one is refused and left untouched.
fn open(storage: &web_sys::Storage) -> StoreResult<SaveDocument> {
    let text = storage.get_item(KEY).map_err(unavailable)?;
    match document::open(text.as_deref()) {
        Err(StoreError::Invalid(reason)) => {
            warn!("Moving the unusable save to `{BROKEN_KEY}`: {reason}");
            if let Some(text) = &text {
                storage.set_item(BROKEN_KEY, text).map_err(unavailable)?;
            }
            storage.remove_item(KEY).map_err(unavailable)?;
            document::open(None)
        }
        other => other,
    }
}

impl Slot for LocalStorage {
    fn update<T>(&self, work: impl FnOnce(&mut SaveDocument) -> StoreResult<T>) -> StoreResult<T> {
        let storage = storage()?;
        let (value, text) = document::apply(open(&storage)?, work)?;
        if let Some(text) = text {
            // Fails without changing the old item, for example when the quota is full.
            storage.set_item(KEY, &text).map_err(unavailable)?;
        }
        Ok(value)
    }
}
