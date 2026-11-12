//! Account provisioning and authoritative drive records for the private test market.
use super::{SqliteStore, profile};
use crate::drive_proof::{DriveProof, DriveTicket, MAX_DRIVE_TICKS};
use crate::storage::{PlayerName, Profile, ProfileId, StoreError, StoreResult};
use crate::vehicles::{GarageAction, GarageRequest};
use bevy::tasks::BoxedFuture;
use rusqlite::{OptionalExtension, TransactionBehavior, params};

fn rejected(message: &str) -> StoreError {
    StoreError::Rejected(message.into())
}
pub fn secret() -> StoreResult<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| StoreError::Unavailable(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

impl SqliteStore {
    /// Only the operator CLI can provision accounts. No public registration or import.
    pub fn provision(
        &self,
        name: PlayerName,
        coins: u64,
    ) -> BoxedFuture<'static, StoreResult<String>> {
        self.run(move |conn| {
            let coins = i64::try_from(coins).map_err(|_| rejected("Initial balance too large"))?;
            let token = secret()?;
            let id = secret()?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO profiles VALUES (?1, ?2, ?3, CAST(unixepoch('subsec') * 1000 AS INTEGER))",
                params![id, name.as_str(), coins],
            )?;
            tx.execute(
                "INSERT INTO market_accounts(token, profile_id) VALUES (?1, ?2)",
                params![token, id],
            )?;
            tx.commit()?;
            Ok(token)
        })
    }

    pub fn authenticate(
        &self,
        token: String,
        client: String,
        login: bool,
    ) -> BoxedFuture<'static, StoreResult<Profile>> {
        self.run(move |conn| {
            if token.len() != 64 || client.len() != 32 { return Err(rejected("Invalid credentials")); }
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let (id, current): (String, Option<String>) = tx.query_row(
                "SELECT profile_id, client FROM market_accounts WHERE token = ?1", [&token],
                |r| Ok((r.get(0)?, r.get(1)?))).optional()?.ok_or_else(|| rejected("Invalid credentials"))?;
            if current.as_deref() != Some(&client) {
                if !login { return Err(rejected("Session replaced. Reconnect this client.")); }
                tx.execute("UPDATE market_accounts SET client = ?2 WHERE token = ?1", params![token, client])?;
                // A new launch abandons an unfinished, unsubmitted drive. Completed
                // credits and garage transaction receipts survive reconnects.
                tx.execute("UPDATE market_drives SET completed = 2 WHERE profile_id = ?1 AND completed = 0", [&id])?;
            }
            let (name, coins) = tx.query_row("SELECT name, coins FROM profiles WHERE id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let result = profile(id, name, coins)?;
            tx.commit()?; Ok(result)
        })
    }

    pub fn market_idle(&self, owner: ProfileId) -> BoxedFuture<'static, StoreResult<()>> {
        self.run(move |conn| {
            let count: i64 = conn.query_row(
                "SELECT count(*) FROM market_drives WHERE profile_id = ?1 AND completed = 0",
                [&owner.0],
                |r| r.get(0),
            )?;
            if count != 0 {
                return Err(rejected(
                    "Finish the current drive before changing vehicles",
                ));
            }
            Ok(())
        })
    }

    pub fn issue_drive(
        &self,
        owner: ProfileId,
        id: String,
    ) -> BoxedFuture<'static, StoreResult<DriveTicket>> {
        self.run(move |conn| {
            if id.len() != 32 { return Err(rejected("Invalid drive ID")); }
            let prior: Option<(String, String, i64)> = conn.query_row("SELECT profile_id, ticket, completed FROM market_drives WHERE id = ?1", [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
            if let Some((profile, data, completed)) = prior {
                if profile != owner.0 || completed != 0 { return Err(rejected("Drive ID already used")); }
                return serde_json::from_str(&data).map_err(|e| StoreError::Invalid(e.to_string()));
            }
            let garage = super::garage::transact(conn, &owner, &GarageRequest::new(GarageAction::Refresh { page: 0 }))?;
            let vehicle = garage.vehicles.into_iter().find(|v| v.id == garage.active).ok_or_else(|| rejected("Active vehicle missing"))?;
            let mut bytes = [0; 8]; getrandom::fill(&mut bytes).map_err(|e| StoreError::Unavailable(e.to_string()))?;
            let ticket = DriveTicket { id: id.clone(), seed: u64::from_le_bytes(bytes), vehicle };
            let data = serde_json::to_string(&ticket).map_err(|e| StoreError::Invalid(e.to_string()))?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let count: i64 = tx.query_row("SELECT count(*) FROM market_drives WHERE profile_id = ?1 AND completed = 0", [&owner.0], |r| r.get(0))?;
            if count != 0 { return Err(rejected("A drive is already in progress")); }
            tx.execute("INSERT INTO market_drives(id, profile_id, ticket, started) VALUES (?1, ?2, ?3, CAST(unixepoch('subsec') * 1000 AS INTEGER))", params![id, owner.0, data])?;
            tx.commit()?; Ok(ticket)
        })
    }

    pub fn finish_drive(
        &self,
        owner: ProfileId,
        proof: DriveProof,
    ) -> BoxedFuture<'static, StoreResult<Profile>> {
        self.run(move |conn| {
            if proof.frames.len() > MAX_DRIVE_TICKS { return Err(rejected("Drive proof too large")); }
            let (data, started, completed, previous): (String, i64, i64, Option<String>) = conn.query_row(
                "SELECT ticket, started, completed, proof FROM market_drives WHERE id = ?1 AND profile_id = ?2",
                params![proof.ticket, owner.0], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?.ok_or_else(|| rejected("Unknown drive"))?;
            let encoded = serde_json::to_string(&proof).map_err(|e| StoreError::Invalid(e.to_string()))?;
            let coins = if completed == 1 {
                if previous.as_deref() != Some(&encoded) { return Err(rejected("Drive already submitted with different inputs")); }
                0
            } else {
                if completed != 0 { return Err(rejected("Drive was abandoned on reconnect")); }
                let now: i64 = conn.query_row("SELECT CAST(unixepoch('subsec') * 1000 AS INTEGER)", [], |r| r.get(0))?;
                if proof.frames.len() as i64 * 1000 > (now - started).max(0) * 64 {
                    return Err(rejected("Drive ran faster than real time"));
                }
                let ticket: DriveTicket = serde_json::from_str(&data).map_err(|e| StoreError::Invalid(e.to_string()))?;
                crate::drive_proof::verify(&ticket, &proof).map_err(StoreError::Rejected)?
            };
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let (name, balance): (String, i64) = tx.query_row("SELECT name, coins FROM profiles WHERE id = ?1", [&owner.0], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let balance = balance.checked_add(coins as i64).ok_or_else(|| rejected("Balance overflow"))?;
            tx.execute("UPDATE profiles SET coins = ?2 WHERE id = ?1", params![owner.0, balance])?;
            tx.execute("UPDATE market_drives SET completed = 1, proof = ?2 WHERE id = ?1", params![proof.ticket, encoded])?;
            tx.commit()?; profile(owner.0, name, balance)
        })
    }
}
