//! The whole save as one plain, versioned document, for stores without SQL.
//!
//! The browser keeps this document in `localStorage` (see `storage/browser.rs`) and
//! tests keep it in memory. Each operation opens the stored text, applies the
//! change and returns the new text, so a failed operation changes nothing. The
//! garage rules mirror `sqlite/garage.rs`; the scenarios in [`super::contract`]
//! hold both implementations to the same behaviour (ADR 0011).
use std::future::ready;

use bevy::tasks::BoxedFuture;
use serde::{Deserialize, Serialize};

use super::{
    PlayerName, Profile, ProfileId, ProfileStore, RunCredit, RunId, StoreError, StoreResult,
};
use crate::vehicles::{
    CATALOG_VERSION, GarageAction, GarageRequest, GarageSnapshot, Model, Vehicle,
};

const FORMAT: &str = "car-drive-save";
pub const VERSION: i64 = 1;
/// Refused before parsing. Thousands of vehicles fit in far less.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
/// Receipts and credited runs only guard retries of the latest operations, so
/// the document keeps the most recent ones and stays small.
const KEPT_RECEIPTS: usize = 256;
const MAX_ID_LEN: usize = 64;
const LISTINGS_PER_PAGE: usize = 20;

/// The version header, read before the rest so that a newer save is reported
/// as newer rather than as invalid.
#[derive(Deserialize)]
struct Header {
    format: String,
    version: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveDocument {
    format: String,
    version: i64,
    /// In creation order; the first one is the local player.
    profiles: Vec<ProfileRecord>,
    /// The most recent credited run IDs, oldest first.
    credited: Vec<String>,
    /// Every vehicle in purchase order; listings have no owner.
    vehicles: Vec<VehicleRecord>,
    garages: Vec<GarageRecord>,
    /// The most recent garage transactions, oldest first.
    receipts: Vec<Receipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileRecord {
    id: String,
    name: String,
    coins: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VehicleRecord {
    owner: Option<String>,
    vehicle: Vehicle,
}

/// Exists once the starter car was granted, even after that car is sold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GarageRecord {
    profile: String,
    active: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    id: String,
    profile: String,
    request: GarageRequest,
}

fn invalid(reason: impl ToString) -> StoreError {
    StoreError::Invalid(reason.to_string())
}

fn reject(message: &str) -> StoreError {
    StoreError::Rejected(message.into())
}

fn new_id() -> String {
    RunId::random().0
}

/// The stored text as a document; no text is an empty save.
pub fn open(text: Option<&str>) -> StoreResult<SaveDocument> {
    match text {
        None => Ok(SaveDocument::new()),
        Some(text) => SaveDocument::decode(text),
    }
}

/// Applies `work` to the document and returns its result together with the new
/// text, or `None` when nothing changed. On an error nothing is to be written.
pub fn apply<T>(
    mut document: SaveDocument,
    work: impl FnOnce(&mut SaveDocument) -> StoreResult<T>,
) -> StoreResult<(T, Option<String>)> {
    let before = document.clone();
    let value = work(&mut document)?;
    let text = if document == before {
        None
    } else {
        Some(document.encode()?)
    };
    Ok((value, text))
}

/// Where the encoded document lives: memory in tests, `localStorage` in the
/// browser.
pub trait Slot: Send + Sync + 'static {
    /// Opens the stored document, runs `work` through [`apply`] and writes the
    /// new text, if any, before returning.
    fn update<T>(&self, work: impl FnOnce(&mut SaveDocument) -> StoreResult<T>) -> StoreResult<T>;
}

/// A [`ProfileStore`] over one document. Every operation finishes before its
/// future is returned.
#[derive(Clone, Default)]
pub struct DocumentStore<S>(pub S);

impl<S: Slot> DocumentStore<S> {
    fn answer<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut SaveDocument) -> StoreResult<T>,
    ) -> BoxedFuture<'static, StoreResult<T>> {
        Box::pin(ready(self.0.update(work)))
    }
}

impl<S: Slot> ProfileStore for DocumentStore<S> {
    fn load(&self) -> BoxedFuture<'static, StoreResult<Option<Profile>>> {
        self.answer(|d| d.first_profile())
    }

    fn create(&self, name: PlayerName) -> BoxedFuture<'static, StoreResult<Profile>> {
        self.answer(|d| Ok(d.create(name)))
    }

    fn credit(&self, credit: RunCredit) -> BoxedFuture<'static, StoreResult<Profile>> {
        self.answer(move |d| d.credit(&credit))
    }

    fn garage(
        &self,
        profile: ProfileId,
        request: GarageRequest,
    ) -> BoxedFuture<'static, StoreResult<GarageSnapshot>> {
        self.answer(move |d| d.garage(&profile, &request))
    }
}

impl SaveDocument {
    fn new() -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            ..Self::default()
        }
    }

    /// Parses and validates untrusted text: the item may have been edited.
    pub fn decode(text: &str) -> StoreResult<Self> {
        if text.len() > MAX_BYTES {
            return Err(invalid("save is too large"));
        }
        let header: Header = serde_json::from_str(text).map_err(invalid)?;
        if header.format != FORMAT {
            return Err(invalid("not a Car Drive save"));
        }
        if header.version > VERSION {
            return Err(StoreError::NewerSchema {
                found: header.version,
                supported: VERSION,
            });
        }
        if header.version != VERSION {
            return Err(invalid(format!("unknown save version {}", header.version)));
        }
        let document: Self = serde_json::from_str(text).map_err(invalid)?;
        document.validate()?;
        Ok(document)
    }

    pub fn encode(&self) -> StoreResult<String> {
        let text = serde_json::to_string(self).map_err(invalid)?;
        if text.len() > MAX_BYTES {
            return Err(StoreError::Unavailable("save is full".into()));
        }
        Ok(text)
    }

    fn validate(&self) -> StoreResult<()> {
        let id_ok = |id: &str| !id.is_empty() && id.len() <= MAX_ID_LEN;
        let mut profiles = std::collections::HashSet::new();
        for profile in &self.profiles {
            if !id_ok(&profile.id) || !profiles.insert(profile.id.as_str()) {
                return Err(invalid("invalid profile ID"));
            }
            profile.to_profile()?;
        }
        let mut vehicles = std::collections::HashSet::new();
        for record in &self.vehicles {
            record.vehicle.validate().map_err(invalid)?;
            if !vehicles.insert(record.vehicle.id.as_str()) {
                return Err(invalid("duplicate vehicle ID"));
            }
            if record
                .owner
                .as_deref()
                .is_some_and(|o| !profiles.contains(o))
            {
                return Err(invalid("vehicle owner not found"));
            }
        }
        let mut garages = std::collections::HashSet::new();
        for garage in &self.garages {
            if !profiles.contains(garage.profile.as_str())
                || !garages.insert(garage.profile.as_str())
            {
                return Err(invalid("invalid garage"));
            }
            if !self
                .vehicles
                .iter()
                .any(|r| r.vehicle.id == garage.active && r.owner.as_ref() == Some(&garage.profile))
            {
                return Err(invalid("Active vehicle is not owned"));
            }
        }
        if self.credited.iter().any(|run| !id_ok(run))
            || self.receipts.iter().any(|r| {
                !id_ok(&r.id)
                    || r.id != r.request.transaction
                    || !profiles.contains(r.profile.as_str())
            })
        {
            return Err(invalid("invalid receipt"));
        }
        Ok(())
    }

    /// The earliest created profile, if any.
    pub fn first_profile(&self) -> StoreResult<Option<Profile>> {
        self.profiles
            .first()
            .map(ProfileRecord::to_profile)
            .transpose()
    }

    #[cfg(test)]
    pub fn profiles(&self) -> Vec<Profile> {
        self.profiles
            .iter()
            .map(|p| p.to_profile().expect("validated profile"))
            .collect()
    }

    pub fn create(&mut self, name: PlayerName) -> Profile {
        let profile = Profile {
            id: ProfileId(new_id()),
            name,
            coins: 0,
        };
        self.profiles.push(ProfileRecord {
            id: profile.id.0.clone(),
            name: profile.name.as_str().into(),
            coins: 0,
        });
        profile
    }

    /// Adds the coins once per run ID. Use it through [`apply`].
    pub fn credit(&mut self, credit: &RunCredit) -> StoreResult<Profile> {
        if credit.coins > i64::MAX as u64 {
            return Err(invalid("credit too large"));
        }
        let index = self.profile_index(&credit.profile.0)?;
        if !self.credited.contains(&credit.run.0) {
            let total = self.profiles[index]
                .coins
                .checked_add(credit.coins)
                .filter(|c| *c <= i64::MAX as u64)
                .ok_or_else(|| invalid("coin balance overflow"))?;
            self.profiles[index].coins = total;
            remember(&mut self.credited, credit.run.0.clone());
        }
        self.profiles[index].to_profile()
    }

    fn profile_index(&self, id: &str) -> StoreResult<usize> {
        self.profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or(StoreError::UnknownProfile)
    }

    fn vehicle_index(&self, id: &str, owner: Option<&str>) -> StoreResult<usize> {
        self.vehicles
            .iter()
            .position(|r| r.vehicle.id == id && r.owner.as_deref() == owner)
            .ok_or_else(|| reject("Vehicle no longer available or not yours"))
    }

    fn add(&mut self, owner: &str, model: Model) -> String {
        let id = new_id();
        self.vehicles.push(VehicleRecord {
            owner: Some(owner.into()),
            vehicle: Vehicle::new(id.clone(), model),
        });
        id
    }

    /// One garage transaction with the rules of `sqlite/garage.rs::transact`.
    /// Use it through [`apply`]: after an error the document may be partly
    /// changed and must be dropped, like a rolled-back SQLite transaction.
    pub fn garage(
        &mut self,
        owner: &ProfileId,
        request: &GarageRequest,
    ) -> StoreResult<GarageSnapshot> {
        if request.version != CATALOG_VERSION {
            return Err(reject("Prices changed. Refresh and confirm the new quote."));
        }
        if request.transaction.is_empty() || request.transaction.len() > MAX_ID_LEN {
            return Err(reject("Invalid transaction ID"));
        }
        let owner = owner.0.as_str();
        let index = self.profile_index(owner)?;
        let mut coins = self.profiles[index].coins;
        let active = match self.garages.iter().find(|g| g.profile == owner) {
            Some(garage) => garage.active.clone(),
            None => {
                let id = self.add(owner, Model::Corsa);
                self.garages.push(GarageRecord {
                    profile: owner.into(),
                    active: id.clone(),
                });
                id
            }
        };
        let page = match request.action {
            GarageAction::Refresh { page } => page,
            _ => 0,
        };
        let previous = self.receipts.iter().find(|r| r.id == request.transaction);
        if let Some(previous) = previous {
            if previous.profile != owner || previous.request != *request {
                return Err(reject("Transaction ID already used"));
            }
        } else if !matches!(request.action, GarageAction::Refresh { .. }) {
            match &request.action {
                GarageAction::Buy(model) => {
                    debit(&mut coins, model.price())?;
                    self.add(owner, *model);
                }
                GarageAction::Select(id) => {
                    self.vehicle_index(id, Some(owner))?;
                    if let Some(garage) = self.garages.iter_mut().find(|g| g.profile == owner) {
                        garage.active = id.clone();
                    }
                }
                GarageAction::Configure { vehicle, change } => {
                    let car = self.vehicle_index(vehicle, Some(owner))?;
                    let car = &mut self.vehicles[car].vehicle;
                    let cost = car.configure(change).map_err(StoreError::Rejected)?;
                    car.validate().map_err(invalid)?;
                    debit(&mut coins, cost)?;
                }
                GarageAction::Sell(id) => {
                    let car = self.vehicle_index(id, Some(owner))?;
                    let count = self
                        .vehicles
                        .iter()
                        .filter(|r| r.owner.as_deref() == Some(owner))
                        .count();
                    if count <= 1 {
                        return Err(reject("Keep at least one vehicle to drive"));
                    }
                    if *id == active {
                        return Err(reject("Select another vehicle before selling this one"));
                    }
                    coins = coins
                        .checked_add(self.vehicles[car].vehicle.sale_price())
                        .filter(|c| *c <= i64::MAX as u64)
                        .ok_or_else(|| reject("Coin balance limit reached"))?;
                    self.vehicles[car].owner = None;
                }
                GarageAction::BuyListing(id) => {
                    let car = self.vehicle_index(id, None)?;
                    debit(&mut coins, self.vehicles[car].vehicle.market_price())?;
                    self.vehicles[car].owner = Some(owner.into());
                }
                GarageAction::Refresh { .. } => unreachable!(),
            }
            self.profiles[index].coins = coins;
            remember(
                &mut self.receipts,
                Receipt {
                    id: request.transaction.clone(),
                    profile: owner.into(),
                    request: request.clone(),
                },
            );
        }
        self.snapshot(owner, page)
    }

    fn snapshot(&self, owner: &str, page: u32) -> StoreResult<GarageSnapshot> {
        let active = self
            .garages
            .iter()
            .find(|g| g.profile == owner)
            .map(|g| g.active.clone())
            .ok_or_else(|| invalid("garage missing"))?;
        let vehicles: Vec<Vehicle> = self
            .vehicles
            .iter()
            .filter(|r| r.owner.as_deref() == Some(owner))
            .map(|r| r.vehicle.clone())
            .collect();
        if !vehicles.iter().any(|v| v.id == active) {
            return Err(invalid("Active vehicle is not owned"));
        }
        let mut listings: Vec<Vehicle> = self
            .vehicles
            .iter()
            .filter(|r| r.owner.is_none())
            .skip(page as usize * LISTINGS_PER_PAGE)
            .take(LISTINGS_PER_PAGE + 1)
            .map(|r| r.vehicle.clone())
            .collect();
        let more = listings.len() > LISTINGS_PER_PAGE;
        listings.truncate(LISTINGS_PER_PAGE);
        Ok(GarageSnapshot {
            coins: self.profiles[self.profile_index(owner)?].coins,
            active,
            vehicles,
            listings,
            page,
            more,
        })
    }
}

impl ProfileRecord {
    /// Validates the stored values again, as SQLite does on load.
    fn to_profile(&self) -> StoreResult<Profile> {
        let name = PlayerName::parse(&self.name)
            .ok()
            .filter(|parsed| parsed.as_str() == self.name)
            .ok_or_else(|| invalid("invalid player name"))?;
        if self.coins > i64::MAX as u64 {
            return Err(invalid("coin balance too large"));
        }
        Ok(Profile {
            id: ProfileId(self.id.clone()),
            name,
            coins: self.coins,
        })
    }
}

fn debit(coins: &mut u64, price: u64) -> StoreResult<()> {
    *coins = coins
        .checked_sub(price)
        .ok_or_else(|| reject("Not enough coins"))?;
    Ok(())
}

/// Appends and forgets the oldest entries beyond [`KEPT_RECEIPTS`].
fn remember<T>(list: &mut Vec<T>, item: T) {
    list.push(item);
    let excess = list.len().saturating_sub(KEPT_RECEIPTS);
    list.drain(..excess);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(work: impl FnOnce(&mut SaveDocument)) -> String {
        let mut document = open(None).unwrap();
        work(&mut document);
        document.encode().unwrap()
    }

    #[test]
    fn a_saved_document_reopens_unchanged() {
        let mut document = open(None).unwrap();
        let profile = document.create(PlayerName::parse("Zoë").unwrap());
        document
            .garage(
                &profile.id,
                &GarageRequest::new(GarageAction::Refresh { page: 0 }),
            )
            .unwrap();
        let text = document.encode().unwrap();
        assert_eq!(open(Some(&text)).unwrap(), document);
    }

    #[test]
    fn newer_foreign_and_edited_saves_are_refused() {
        assert!(matches!(
            open(Some(
                r#"{"format":"car-drive-save","version":2,"future":true}"#
            )),
            Err(StoreError::NewerSchema {
                found: 2,
                supported: 1
            })
        ));
        for text in [
            "",
            "[]",
            r#"{"format":"moto-devil","version":1}"#,
            r#"{"format":"car-drive-save","version":0}"#,
        ] {
            assert!(
                matches!(open(Some(text)), Err(StoreError::Invalid(_))),
                "{text}"
            );
        }
        let text = saved(|d| {
            d.create(PlayerName::parse("Ada").unwrap());
        });
        for (from, to) in [
            ("\"Ada\"", "\"\""),
            ("\"coins\":0", "\"coins\":9223372036854775808"),
            ("\"credited\":[]", "\"credited\":[],\"extra\":1"),
        ] {
            let edited = text.replace(from, to);
            assert_ne!(edited, text);
            assert!(
                matches!(open(Some(&edited)), Err(StoreError::Invalid(_))),
                "{edited}"
            );
        }
        let oversized = format!("{text}{}", " ".repeat(MAX_BYTES));
        assert!(matches!(
            open(Some(&oversized)),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn a_vehicle_must_belong_to_a_known_profile_and_be_valid() {
        let text = saved(|d| {
            let p = d.create(PlayerName::parse("Ada").unwrap());
            d.garage(
                &p.id,
                &GarageRequest::new(GarageAction::Refresh { page: 0 }),
            )
            .unwrap();
        });
        let document = open(Some(&text)).unwrap();
        let car = &document.vehicles[0].vehicle.id;
        let stolen = text.replace(
            &format!("\"owner\":\"{}\"", document.profiles[0].id),
            "\"owner\":\"nobody\"",
        );
        assert!(matches!(open(Some(&stolen)), Err(StoreError::Invalid(_))));
        let tuned = text.replace("\"motor\":0", "\"motor\":2");
        assert!(matches!(open(Some(&tuned)), Err(StoreError::Invalid(_))));
        let listed = text.replace(&format!("\"active\":\"{car}\""), "\"active\":\"other\"");
        assert!(matches!(open(Some(&listed)), Err(StoreError::Invalid(_))));
    }

    #[test]
    fn a_failed_operation_writes_nothing_and_a_refresh_only_the_starter_grant() {
        let text = saved(|d| {
            d.create(PlayerName::parse("Ada").unwrap());
        });
        let document = open(Some(&text)).unwrap();
        let id = document.profiles()[0].id.clone();
        let failed = apply(document.clone(), |d| {
            d.garage(&id, &GarageRequest::new(GarageAction::Buy(Model::Jesko)))
        });
        assert!(failed.is_err());
        let (_, granted) = apply(document, |d| {
            d.garage(&id, &GarageRequest::new(GarageAction::Refresh { page: 0 }))
        })
        .unwrap();
        let granted = granted.expect("the starter grant is written");
        let (_, again) = apply(open(Some(&granted)).unwrap(), |d| {
            d.garage(&id, &GarageRequest::new(GarageAction::Refresh { page: 0 }))
        })
        .unwrap();
        assert_eq!(again, None);
    }

    #[test]
    fn only_recent_receipts_and_runs_are_kept() {
        let mut document = open(None).unwrap();
        let p = document.create(PlayerName::parse("Ada").unwrap());
        let refresh = GarageRequest::new(GarageAction::Refresh { page: 0 });
        let active = document.garage(&p.id, &refresh).unwrap().active;
        for _ in 0..KEPT_RECEIPTS + 10 {
            let run = RunCredit {
                run: RunId::random(),
                profile: p.id.clone(),
                coins: 1,
                proof: None,
            };
            document.credit(&run).unwrap();
            let select = GarageRequest::new(GarageAction::Select(active.clone()));
            document.garage(&p.id, &select).unwrap();
        }
        assert_eq!(document.credited.len(), KEPT_RECEIPTS);
        assert_eq!(document.receipts.len(), KEPT_RECEIPTS);
        assert_eq!(document.profiles[0].coins, (KEPT_RECEIPTS + 10) as u64);
    }
}
