//! Garage and credit scenarios every [`ProfileStore`] with a local market must
//! pass. They run against SQLite and against the save document behind the
//! memory and browser stores, so the two rule implementations cannot drift
//! apart unnoticed (ADR 0011).
use bevy::tasks::block_on;

use super::memory::MemoryStore;
use super::sqlite::{SqliteStore, TempDir};
use super::{PlayerName, ProfileId, ProfileStore, RunCredit, RunId};
use crate::vehicles::{Change, GarageAction, GarageRequest, GarageSnapshot, Model, Paint, Upgrade};

fn account(store: &impl ProfileStore, name: &str, coins: u64) -> ProfileId {
    let p = block_on(store.create(PlayerName::parse(name).unwrap())).unwrap();
    block_on(store.credit(RunCredit {
        run: RunId::random(),
        profile: p.id.clone(),
        coins,
        proof: None,
    }))
    .unwrap();
    p.id
}

fn act(store: &impl ProfileStore, owner: &ProfileId, action: GarageAction) -> GarageSnapshot {
    block_on(store.garage(owner.clone(), GarageRequest::new(action))).unwrap()
}

fn refresh(store: &impl ProfileStore, owner: &ProfileId) -> GarageSnapshot {
    act(store, owner, GarageAction::Refresh { page: 0 })
}

fn other_car(garage: &GarageSnapshot, not: &str) -> String {
    garage
        .vehicles
        .iter()
        .find(|v| v.id != not)
        .unwrap()
        .id
        .clone()
}

/// A tuned car listed by one player keeps its tuning for the buyer; the seller
/// is paid once, and reopening grants no second starter car.
fn market_transfers_custom_car_and_pays_seller_exactly_once<S: ProfileStore>(
    store: &S,
    reopen: impl FnOnce() -> S,
) {
    let a = account(store, "Ada", 100_000);
    let b = account(store, "Bob", 100_000);
    let id = refresh(store, &a).active;
    for change in [
        Change::Buy(Upgrade::Motor1),
        Change::Buy(Upgrade::Carbon),
        Change::Paint(Paint::Red),
    ] {
        let vehicle = id.clone();
        act(store, &a, GarageAction::Configure { vehicle, change });
    }
    let bought = act(store, &a, GarageAction::Buy(Model::BobbyCar));
    act(store, &a, GarageAction::Select(other_car(&bought, &id)));
    let sale = GarageRequest::new(GarageAction::Sell(id.clone()));
    let sold = block_on(store.garage(a.clone(), sale.clone())).unwrap();
    assert_eq!(sold.coins, 100_000 - 2990 - 55 + 17595);
    assert_eq!(sold, block_on(store.garage(a.clone(), sale)).unwrap());
    let purchased = act(store, &b, GarageAction::BuyListing(id.clone()));
    let car = purchased.vehicles.iter().find(|v| v.id == id).unwrap();
    assert_eq!((car.paint, car.motor, car.carbon), (Paint::Red, 1, true));
    assert_eq!(purchased.coins, 100_000 - 21793);
    let reopened = reopen();
    let after = refresh(&reopened, &a);
    assert_eq!(after.coins, sold.coins);
    assert_eq!(after.vehicles.len(), 1);
    assert!(after.listings.is_empty());
}

/// Rejected operations change nothing, an exact retry returns the snapshot, and
/// a transaction ID cannot be reused for something else.
fn rejection_is_atomic_and_transaction_ids_cannot_be_repurposed(store: &impl ProfileStore) {
    let a = account(store, "Ada", 55);
    let original = refresh(store, &a);
    for action in [
        GarageAction::Sell(original.active.clone()),
        GarageAction::Buy(Model::Jesko),
        GarageAction::Configure {
            vehicle: original.active.clone(),
            change: Change::Buy(Upgrade::Motor2),
        },
        GarageAction::Select("not-a-car".into()),
        GarageAction::BuyListing(original.active.clone()),
    ] {
        assert!(block_on(store.garage(a.clone(), GarageRequest::new(action))).is_err());
        assert_eq!(refresh(store, &a), original);
    }
    let mut request = GarageRequest::new(GarageAction::Buy(Model::BobbyCar));
    let bought = block_on(store.garage(a.clone(), request.clone())).unwrap();
    assert_eq!(bought.coins, 0);
    assert_eq!(
        block_on(store.garage(a.clone(), request.clone())).unwrap(),
        bought
    );
    let b = account(store, "Bob", 1000);
    assert!(block_on(store.garage(b, request.clone())).is_err());
    request.action = GarageAction::Buy(Model::Corsa);
    assert!(block_on(store.garage(a.clone(), request)).is_err());
    let mut stale = GarageRequest::new(GarageAction::Refresh { page: 0 });
    stale.version += 1;
    assert!(block_on(store.garage(a, stale)).is_err());
}

/// The active car cannot be sold; a sold car can be bought back from the market
/// at the market price.
fn the_active_car_stays_and_a_sold_car_can_be_bought_back(store: &impl ProfileStore) {
    let a = account(store, "Ada", 1000);
    let start = refresh(store, &a);
    let bought = act(store, &a, GarageAction::Buy(Model::BobbyCar));
    let bobby = other_car(&bought, &start.active);
    let active = GarageRequest::new(GarageAction::Sell(start.active.clone()));
    assert!(block_on(store.garage(a.clone(), active)).is_err());
    let sold = act(store, &a, GarageAction::Sell(bobby.clone()));
    assert_eq!(sold.coins, 1000 - 55 + 38);
    assert_eq!(sold.listings.len(), 1);
    assert_eq!(sold.vehicles.len(), 1);
    let back = act(store, &a, GarageAction::BuyListing(bobby.clone()));
    assert_eq!(back.coins, sold.coins - 47);
    assert!(back.listings.is_empty());
    assert!(back.vehicles.iter().any(|v| v.id == bobby));
}

/// Listings come in pages of 20, oldest first.
fn listings_are_paged(store: &impl ProfileStore) {
    let a = account(store, "Ada", 10_000);
    let start = refresh(store, &a);
    let mut cars = Vec::new();
    for _ in 0..21 {
        let bought = act(store, &a, GarageAction::Buy(Model::BobbyCar));
        cars.push(bought.vehicles.last().unwrap().id.clone());
    }
    assert!(!cars.contains(&start.active));
    for car in &cars {
        act(store, &a, GarageAction::Sell(car.clone()));
    }
    let first = refresh(store, &a);
    assert_eq!((first.listings.len(), first.more), (20, true));
    assert_eq!(first.listings[0].id, cars[0]);
    let second = act(store, &a, GarageAction::Refresh { page: 1 });
    assert_eq!((second.listings.len(), second.more), (1, false));
    assert_eq!(second.listings[0].id, cars[20]);
}

/// A credit counts once per run, also after reopening; unknown profiles fail.
fn credits_count_once_per_run<S: ProfileStore>(store: &S, reopen: impl FnOnce() -> S) {
    let a = account(store, "Ada", 5);
    let credit = RunCredit {
        run: RunId::random(),
        profile: a.clone(),
        coins: 7,
        proof: None,
    };
    assert_eq!(block_on(store.credit(credit.clone())).unwrap().coins, 12);
    let reopened = reopen();
    assert_eq!(block_on(reopened.credit(credit.clone())).unwrap().coins, 12);
    let loaded = block_on(reopened.load()).unwrap().unwrap();
    assert_eq!(
        (loaded.id, loaded.name.as_str(), loaded.coins),
        (a, "Ada", 12)
    );
    let stranger = RunCredit {
        profile: ProfileId("nobody".into()),
        run: RunId::random(),
        ..credit
    };
    assert!(block_on(reopened.credit(stranger)).is_err());
}

/// Runs each scenario against a fresh SQLite file and a fresh memory store.
macro_rules! contract {
    ($($scenario:ident $(, $reopen:tt)?;)*) => {
        mod sqlite {
            use super::*;
            $(#[test]
            fn $scenario() {
                let dir = TempDir::new();
                let store = SqliteStore::new(Some(dir.file()));
                super::$scenario(&store $(, contract!(@reopen $reopen || SqliteStore::new(Some(dir.file()))))?);
            })*
        }
        mod document {
            use super::*;
            $(#[test]
            fn $scenario() {
                let store = MemoryStore::default();
                super::$scenario(&store $(, contract!(@reopen $reopen || store.clone()))?);
            })*
        }
    };
    (@reopen reopen $make:expr) => { $make };
}

contract! {
    market_transfers_custom_car_and_pays_seller_exactly_once, reopen;
    rejection_is_atomic_and_transaction_ids_cannot_be_repurposed;
    the_active_car_stays_and_a_sold_car_can_be_bought_back;
    listings_are_paged;
    credits_count_once_per_run, reopen;
}
