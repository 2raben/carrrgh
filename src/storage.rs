//! The profile storage contract: plain data and asynchronous operations.
//!
//! Game systems only see [`ProfileStore`]. SQLite stays behind it in
//! [`sqlite`], so another backend (for example browser storage) can replace it.
use std::fmt;

use crate::vehicles::{GarageRequest, GarageSnapshot};
use bevy::tasks::BoxedFuture;
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub mod memory;
#[cfg(not(target_arch = "wasm32"))]
pub mod sqlite;

pub const NAME_MAX_CHARS: usize = 16;

/// Stable profile identity, assigned by the store.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(pub String);

/// A trimmed name of 1–16 characters without control characters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerName(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    ControlCharacter,
}

impl PlayerName {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        let name = raw.trim();
        if name.is_empty() {
            Err(NameError::Empty)
        } else if name.chars().count() > NAME_MAX_CHARS {
            Err(NameError::TooLong)
        } else if name.chars().any(char::is_control) {
            Err(NameError::ControlCharacter)
        } else {
            Ok(Self(name.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Enter a name"),
            Self::TooLong => write!(f, "Use at most {NAME_MAX_CHARS} characters"),
            Self::ControlCharacter => write!(f, "Use only printable characters"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub name: PlayerName,
    pub coins: u64,
}

/// Identifies one finished drive, so retrying its credit cannot count twice.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RunId(pub String);

impl RunId {
    pub fn random() -> Self {
        Self(format!("{:032x}", fastrand::u128(..)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunCredit {
    pub run: RunId,
    pub profile: ProfileId,
    pub coins: u64,
    pub proof: Option<crate::drive_proof::DriveProof>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreError {
    /// Storage could not be reached, read or written; a later attempt may work.
    Unavailable(String),
    /// The data was written by a newer version of the game.
    NewerSchema {
        found: i64,
        supported: i64,
    },
    /// The stored data is not a valid Car Drive save.
    Invalid(String),
    UnknownProfile,
    Rejected(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "storage unavailable: {reason}"),
            Self::NewerSchema { found, supported } => write!(
                f,
                "save is from a newer version (schema {found}, supported {supported})"
            ),
            Self::Invalid(reason) => write!(f, "save is not valid: {reason}"),
            Self::UnknownProfile => write!(f, "profile not found"),
            Self::Rejected(reason) => write!(f, "{reason}"),
        }
    }
}

pub type StoreResult<T> = Result<T, StoreError>;

/// Persists player profiles. Operations return futures so that native file
/// access and asynchronous browser storage fit the same contract; game systems
/// never wait for them.
pub trait ProfileStore: Send + Sync + 'static {
    fn online(&self) -> bool {
        false
    }
    fn begin_drive(
        &self,
        _request: String,
    ) -> BoxedFuture<'static, StoreResult<Option<crate::drive_proof::DriveTicket>>> {
        Box::pin(async { Ok(None) })
    }
    /// The earliest created profile, if any.
    fn load(&self) -> BoxedFuture<'static, StoreResult<Option<Profile>>>;
    /// Creates a profile with no coins.
    fn create(&self, name: PlayerName) -> BoxedFuture<'static, StoreResult<Profile>>;
    /// Adds the coins once per run id and returns the updated profile.
    fn credit(&self, credit: RunCredit) -> BoxedFuture<'static, StoreResult<Profile>>;
    fn garage(
        &self,
        _profile: ProfileId,
        _request: GarageRequest,
    ) -> BoxedFuture<'static, StoreResult<GarageSnapshot>> {
        Box::pin(async {
            Err(StoreError::Rejected(
                "Garage unavailable in this store".into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_limited() {
        assert_eq!(PlayerName::parse("  Ada ").unwrap().as_str(), "Ada");
        assert_eq!(PlayerName::parse(" \t "), Err(NameError::Empty));
        assert!(PlayerName::parse("Ä".repeat(16).as_str()).is_ok());
        assert_eq!(
            PlayerName::parse("x".repeat(17).as_str()),
            Err(NameError::TooLong)
        );
        assert_eq!(
            PlayerName::parse("A\u{7}B"),
            Err(NameError::ControlCharacter)
        );
    }

    #[test]
    fn run_ids_are_distinct() {
        assert_ne!(RunId::random(), RunId::random());
        assert_eq!(RunId::random().0.len(), 32);
    }
}
