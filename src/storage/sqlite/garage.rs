//! Schema 2 and atomic garage transactions, shared by local and network stores.
use crate::storage::{ProfileId, StoreError, StoreResult};
use crate::vehicles::{
    CATALOG_VERSION, GarageAction, GarageRequest, GarageSnapshot, Model, Vehicle,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

pub(super) const SCHEMA_V2: &str = "
    CREATE TABLE save_realm (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        realm TEXT NOT NULL CHECK (realm IN ('local', 'market'))
    ) STRICT;
    CREATE TABLE vehicles (
        id TEXT PRIMARY KEY NOT NULL,
        owner TEXT REFERENCES profiles(id),
        data TEXT NOT NULL
    ) STRICT;
    CREATE INDEX vehicle_owner ON vehicles(owner);
    CREATE TABLE garages (
        profile_id TEXT PRIMARY KEY NOT NULL REFERENCES profiles(id),
        active TEXT NOT NULL REFERENCES vehicles(id)
    ) STRICT;
    CREATE TABLE garage_transactions (
        id TEXT PRIMARY KEY NOT NULL,
        profile_id TEXT NOT NULL REFERENCES profiles(id),
        request TEXT NOT NULL
    ) STRICT;
    CREATE TABLE market_accounts (
        token TEXT PRIMARY KEY NOT NULL,
        profile_id TEXT NOT NULL UNIQUE REFERENCES profiles(id),
        client TEXT
    ) STRICT;
    CREATE TABLE market_drives (
        id TEXT PRIMARY KEY NOT NULL,
        profile_id TEXT NOT NULL REFERENCES profiles(id),
        ticket TEXT NOT NULL,
        started INTEGER NOT NULL,
        proof TEXT,
        completed INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX market_drive_owner ON market_drives(profile_id, completed);
";

fn invalid(error: impl ToString) -> StoreError {
    StoreError::Invalid(error.to_string())
}
fn reject(message: &str) -> StoreError {
    StoreError::Rejected(message.into())
}

fn vehicle(connection: &Connection, id: &str, owner: Option<&str>) -> StoreResult<Vehicle> {
    let data: String = connection
        .query_row(
            "SELECT data FROM vehicles WHERE id = ?1 AND owner IS ?2",
            params![id, owner],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| reject("Vehicle no longer available or not yours"))?;
    decode(id, &data)
}

fn decode(id: &str, data: &str) -> StoreResult<Vehicle> {
    let car: Vehicle = serde_json::from_str(data).map_err(invalid)?;
    car.validate().map_err(invalid)?;
    if car.id != id {
        return Err(invalid("Vehicle ID mismatch"));
    }
    Ok(car)
}

fn save(connection: &Connection, car: &Vehicle) -> StoreResult<()> {
    car.validate().map_err(invalid)?;
    connection.execute(
        "UPDATE vehicles SET data = ?2 WHERE id = ?1",
        params![car.id, serde_json::to_string(car).map_err(invalid)?],
    )?;
    Ok(())
}

fn add(connection: &Connection, owner: &str, model: Model) -> StoreResult<String> {
    let id: String = connection.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
    let car = Vehicle::new(id.clone(), model);
    connection.execute(
        "INSERT INTO vehicles(id, owner, data) VALUES (?1, ?2, ?3)",
        params![id, owner, serde_json::to_string(&car).map_err(invalid)?],
    )?;
    Ok(id)
}

fn balance(connection: &Connection, owner: &str) -> StoreResult<u64> {
    let coins: i64 = connection
        .query_row("SELECT coins FROM profiles WHERE id = ?1", [owner], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or(StoreError::UnknownProfile)?;
    u64::try_from(coins).map_err(invalid)
}

fn debit(coins: &mut u64, price: u64) -> StoreResult<()> {
    *coins = coins
        .checked_sub(price)
        .ok_or_else(|| reject("Not enough coins"))?;
    Ok(())
}

pub(super) fn transact(
    connection: &mut Connection,
    owner: &ProfileId,
    request: &GarageRequest,
) -> StoreResult<GarageSnapshot> {
    if request.version != CATALOG_VERSION {
        return Err(reject("Prices changed. Refresh and confirm the new quote."));
    }
    if request.transaction.is_empty() || request.transaction.len() > 64 {
        return Err(reject("Invalid transaction ID"));
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut coins = balance(&tx, &owner.0)?;
    let active: Option<String> = tx
        .query_row(
            "SELECT active FROM garages WHERE profile_id = ?1",
            [&owner.0],
            |r| r.get(0),
        )
        .optional()?;
    let active = match active {
        Some(id) => id,
        None => {
            let id = add(&tx, &owner.0, Model::Corsa)?;
            tx.execute("INSERT INTO garages VALUES (?1, ?2)", params![owner.0, id])?;
            id
        }
    };
    // The grant marker lives in garages, independently of the starter's later owner.
    let encoded = serde_json::to_string(request).map_err(invalid)?;
    let previous: Option<(String, String)> = tx
        .query_row(
            "SELECT profile_id, request FROM garage_transactions WHERE id = ?1",
            [&request.transaction],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let page = match request.action {
        GarageAction::Refresh { page } => page,
        _ => 0,
    };
    if let Some((profile, prior)) = previous {
        if profile != owner.0 || prior != encoded {
            return Err(reject("Transaction ID already used"));
        }
    } else if !matches!(request.action, GarageAction::Refresh { .. }) {
        match &request.action {
            GarageAction::Buy(model) => {
                debit(&mut coins, model.price())?;
                add(&tx, &owner.0, *model)?;
            }
            GarageAction::Select(id) => {
                vehicle(&tx, id, Some(&owner.0))?;
                tx.execute(
                    "UPDATE garages SET active = ?2 WHERE profile_id = ?1",
                    params![owner.0, id],
                )?;
            }
            GarageAction::Configure {
                vehicle: id,
                change,
            } => {
                let mut car = vehicle(&tx, id, Some(&owner.0))?;
                let cost = car.configure(change).map_err(StoreError::Rejected)?;
                debit(&mut coins, cost)?;
                save(&tx, &car)?;
            }
            GarageAction::Sell(id) => {
                let car = vehicle(&tx, id, Some(&owner.0))?;
                let count: i64 = tx.query_row(
                    "SELECT count(*) FROM vehicles WHERE owner = ?1",
                    [&owner.0],
                    |r| r.get(0),
                )?;
                if count <= 1 {
                    return Err(reject("Keep at least one vehicle to drive"));
                }
                if *id == active {
                    return Err(reject("Select another vehicle before selling this one"));
                }
                coins = coins
                    .checked_add(car.sale_price())
                    .filter(|c| *c <= i64::MAX as u64)
                    .ok_or_else(|| reject("Coin balance limit reached"))?;
                tx.execute("UPDATE vehicles SET owner = NULL WHERE id = ?1", [id])?;
            }
            GarageAction::BuyListing(id) => {
                let car = vehicle(&tx, id, None)?;
                debit(&mut coins, car.market_price())?;
                tx.execute(
                    "UPDATE vehicles SET owner = ?2 WHERE id = ?1",
                    params![id, owner.0],
                )?;
            }
            GarageAction::Refresh { .. } => unreachable!(),
        }
        tx.execute(
            "UPDATE profiles SET coins = ?2 WHERE id = ?1",
            params![owner.0, coins as i64],
        )?;
        tx.execute(
            "INSERT INTO garage_transactions VALUES (?1, ?2, ?3)",
            params![request.transaction, owner.0, encoded],
        )?;
    }
    let snapshot = snapshot(&tx, &owner.0, page)?;
    tx.commit()?;
    Ok(snapshot)
}

fn snapshot(connection: &Connection, owner: &str, page: u32) -> StoreResult<GarageSnapshot> {
    let active = connection.query_row(
        "SELECT active FROM garages WHERE profile_id = ?1",
        [owner],
        |r| r.get(0),
    )?;
    let mut query =
        connection.prepare("SELECT id, data FROM vehicles WHERE owner = ?1 ORDER BY rowid")?;
    let vehicles = query
        .query_map([owner], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .map(|r| {
            let (id, data) = r?;
            decode(&id, &data)
        })
        .collect::<StoreResult<Vec<_>>>()?;
    if !vehicles.iter().any(|v| v.id == active) {
        return Err(invalid("Active vehicle is not owned"));
    }
    let mut query = connection.prepare(
        "SELECT id, data FROM vehicles WHERE owner IS NULL ORDER BY rowid LIMIT 21 OFFSET ?1",
    )?;
    let mut listings = query
        .query_map([i64::from(page) * 20], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .map(|r| {
            let (id, data) = r?;
            decode(&id, &data)
        })
        .collect::<StoreResult<Vec<_>>>()?;
    let more = listings.len() > 20;
    listings.truncate(20);
    Ok(GarageSnapshot {
        coins: balance(connection, owner)?,
        active,
        vehicles,
        listings,
        page,
        more,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::{APPLICATION_ID, SCHEMA_V1, SqliteStore, TempDir};
    use crate::storage::{PlayerName, ProfileStore, RunCredit, RunId};
    use bevy::tasks::block_on;

    // Garage rules shared with the save document are tested in `storage::contract`.

    fn account(store: &SqliteStore, name: &str, coins: u64) -> ProfileId {
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
    fn act(store: &SqliteStore, owner: &ProfileId, action: GarageAction) -> GarageSnapshot {
        block_on(store.garage(owner.clone(), GarageRequest::new(action))).unwrap()
    }
    #[test]
    fn concurrent_buyers_cannot_duplicate_a_listing() {
        let dir = TempDir::new();
        let store = SqliteStore::new(Some(dir.file()));
        let seller = account(&store, "Seller", 55);
        let a = account(&store, "A", 100);
        let b = account(&store, "B", 100);
        let garage = act(&store, &seller, GarageAction::Buy(Model::BobbyCar));
        let car = garage
            .vehicles
            .iter()
            .find(|v| v.model == Model::BobbyCar)
            .unwrap()
            .id
            .clone();
        act(&store, &seller, GarageAction::Sell(car.clone()));
        let workers: Vec<_> = [a.clone(), b.clone()]
            .into_iter()
            .map(|owner| {
                let path = dir.file();
                let car = car.clone();
                std::thread::spawn(move || {
                    block_on(
                        SqliteStore::new(Some(path))
                            .garage(owner, GarageRequest::new(GarageAction::BuyListing(car))),
                    )
                })
            })
            .collect();
        let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        let sa = act(&store, &a, GarageAction::Refresh { page: 0 });
        let sb = act(&store, &b, GarageAction::Refresh { page: 0 });
        assert_eq!(sa.coins + sb.coins, 200 - 47);
        assert_eq!(sa.vehicles.len() + sb.vehicles.len(), 3);
    }
    #[test]
    fn schema_one_migrates_and_grants_starter_once() {
        let dir = TempDir::new();
        let conn = Connection::open(dir.file()).unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        conn.pragma_update(None, "application_id", APPLICATION_ID)
            .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO profiles VALUES ('old','Ada',123,0)", [])
            .unwrap();
        drop(conn);
        let store = SqliteStore::new(Some(dir.file()));
        let p = block_on(store.load()).unwrap().unwrap();
        let a = act(&store, &p.id, GarageAction::Refresh { page: 0 });
        let b = act(&store, &p.id, GarageAction::Refresh { page: 0 });
        assert_eq!(a, b);
        assert_eq!(a.coins, 123);
        assert_eq!(a.vehicles.len(), 1);
    }
}
