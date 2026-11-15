//! Native profile storage in one SQLite file.
//!
//! The connection opens lazily on the first operation, and blocking work runs on
//! Bevy's I/O task pool. Each credit is one transaction; a table of credited run
//! ids makes repeated credits harmless.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::tasks::{BoxedFuture, IoTaskPool};
use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};

use super::{PlayerName, Profile, ProfileId, ProfileStore, RunCredit, StoreError, StoreResult};
use crate::vehicles::{GarageRequest, GarageSnapshot};

mod garage;
mod market;

/// Overrides the save directory, for development and play tests.
pub const DATA_DIR_ENV: &str = "CAR_DRIVE_DATA_DIR";
const FILE_NAME: &str = "profiles.sqlite3";
/// Marks the file as a Car Drive save ("CarD").
const APPLICATION_ID: i32 = 0x4361_7244;
const SCHEMA_VERSION: i64 = 2;

const SCHEMA_V1: &str = "
    CREATE TABLE profiles (
        id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL,
        coins INTEGER NOT NULL CHECK (coins >= 0),
        created_at INTEGER NOT NULL
    ) STRICT;
    CREATE TABLE credited_runs (
        run_id TEXT PRIMARY KEY NOT NULL,
        profile_id TEXT NOT NULL REFERENCES profiles (id)
    ) STRICT, WITHOUT ROWID;
";

#[derive(Clone)]
pub struct SqliteStore {
    market: bool,
    path: Option<Arc<PathBuf>>,
    connection: Arc<Mutex<Option<Connection>>>,
}

impl SqliteStore {
    /// Without a path, every operation reports storage as unavailable.
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            market: false,
            path: path.map(Arc::new),
            connection: Arc::default(),
        }
    }

    /// Authoritative market files cannot be opened by the local game backend.
    pub fn new_market(path: PathBuf) -> Self {
        Self {
            market: true,
            ..Self::new(Some(path))
        }
    }

    /// `$CAR_DRIVE_DATA_DIR`, otherwise `car-drive` in the platform data directory.
    pub fn default_path() -> Option<PathBuf> {
        std::env::var_os(DATA_DIR_ENV)
            .map(PathBuf::from)
            .or_else(|| dirs::data_dir().map(|dir| dir.join("car-drive")))
            .map(|dir| dir.join(FILE_NAME))
    }

    /// Runs blocking work on the I/O pool, or when polled if no pool exists.
    /// Authoritative market work always runs when polled, on the server's own
    /// worker thread, so it never waits behind blocking client requests that
    /// occupy the shared pool (as in tests that run both ends in one process).
    pub(crate) fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Connection) -> StoreResult<T> + Send + 'static,
    ) -> BoxedFuture<'static, StoreResult<T>> {
        let store = self.clone();
        let job = move || store.with_connection(work);
        match IoTaskPool::try_get().filter(|_| !self.market) {
            Some(pool) => Box::pin(pool.spawn(async move { job() })),
            None => Box::pin(async move { job() }),
        }
    }

    fn with_connection<T>(
        &self,
        work: impl FnOnce(&mut Connection) -> StoreResult<T>,
    ) -> StoreResult<T> {
        let mut slot = self
            .connection
            .lock()
            .map_err(|_| StoreError::Unavailable("storage lock poisoned".into()))?;
        // A failed open leaves the slot empty, so the next operation retries.
        let connection = match &mut *slot {
            Some(connection) => connection,
            empty => empty.insert(self.open()?),
        };
        work(connection)
    }

    fn open(&self) -> StoreResult<Connection> {
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| StoreError::Unavailable("no data directory".into()))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|error| StoreError::Unavailable(error.to_string()))?;
        }
        #[cfg(unix)]
        if self.market {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(path)
                .map_err(|e| StoreError::Unavailable(e.to_string()))?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| StoreError::Unavailable(e.to_string()))?;
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut connection)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let expected = if self.market { "market" } else { "local" };
        let realm: Option<String> = tx
            .query_row(
                "SELECT realm FROM save_realm WHERE singleton = 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        match realm {
            Some(realm) if realm != expected => {
                return Err(StoreError::Invalid(
                    "Local and market saves must stay separate".into(),
                ));
            }
            None => {
                let count: i64 = tx.query_row("SELECT count(*) FROM profiles", [], |r| r.get(0))?;
                if self.market && count > 0 {
                    return Err(StoreError::Invalid(
                        "Cannot import a local save into the market".into(),
                    ));
                }
                tx.execute("INSERT INTO save_realm VALUES (1, ?1)", [expected])?;
            }
            _ => {}
        }
        tx.commit()?;
        Ok(connection)
    }
}

/// Initialises an empty file and refuses files that are not ours or are newer.
/// Refusal never writes, so the original stays recoverable.
fn migrate(connection: &mut Connection) -> StoreResult<()> {
    let (version, application) = header(connection)?;
    if version == 0 && application == 0 {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Re-check under the write lock: another instance may have initialised it.
        if header(&transaction)? == (0, 0) {
            let tables: i64 =
                transaction
                    .query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get(0))?;
            if tables > 0 {
                return Err(StoreError::Invalid("not a Car Drive save".into()));
            }
            transaction.execute_batch(SCHEMA_V1)?;
            transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
            transaction.pragma_update(None, "user_version", 1)?;
        }
        transaction.commit()?;
    }
    let (version, application) = header(connection)?;
    if application != APPLICATION_ID {
        return Err(StoreError::Invalid("not a Car Drive save".into()));
    }
    if version == 1 {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if header(&transaction)?.0 == 1 {
            transaction.execute_batch(garage::SCHEMA_V2)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        transaction.commit()?;
        return Ok(());
    }
    match version {
        SCHEMA_VERSION => Ok(()),
        found if found > SCHEMA_VERSION => Err(StoreError::NewerSchema {
            found,
            supported: SCHEMA_VERSION,
        }),
        found => Err(StoreError::Invalid(format!("unknown schema {found}"))),
    }
}

fn header(connection: &Connection) -> StoreResult<(i64, i32)> {
    let version = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let application = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    Ok((version, application))
}

/// Validates stored values again: the file may have been edited.
fn profile(id: String, name: String, coins: i64) -> StoreResult<Profile> {
    let name = PlayerName::parse(&name)
        .ok()
        .filter(|parsed| parsed.as_str() == name)
        .ok_or_else(|| StoreError::Invalid("invalid player name".into()))?;
    let coins =
        u64::try_from(coins).map_err(|_| StoreError::Invalid("negative coin balance".into()))?;
    Ok(Profile {
        id: ProfileId(id),
        name,
        coins,
    })
}

impl ProfileStore for SqliteStore {
    fn garage(
        &self,
        profile: ProfileId,
        request: GarageRequest,
    ) -> BoxedFuture<'static, StoreResult<GarageSnapshot>> {
        self.run(move |connection| garage::transact(connection, &profile, &request))
    }
    fn load(&self) -> BoxedFuture<'static, StoreResult<Option<Profile>>> {
        self.run(|connection| {
            connection
                .query_row(
                    "SELECT id, name, coins FROM profiles ORDER BY created_at, rowid LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?
                .map(|(id, name, coins)| profile(id, name, coins))
                .transpose()
        })
    }

    fn create(&self, name: PlayerName) -> BoxedFuture<'static, StoreResult<Profile>> {
        self.run(move |connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let id: String =
                transaction.query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))?;
            transaction.execute(
                "INSERT INTO profiles (id, name, coins, created_at) VALUES (?1, ?2, 0, unixepoch())",
                params![id, name.as_str()],
            )?;
            transaction.commit()?;
            Ok(Profile {
                id: ProfileId(id),
                name,
                coins: 0,
            })
        })
    }

    fn credit(&self, credit: RunCredit) -> BoxedFuture<'static, StoreResult<Profile>> {
        self.run(move |connection| {
            let coins = i64::try_from(credit.coins)
                .map_err(|_| StoreError::Invalid("credit too large".into()))?;
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let (name, balance): (String, i64) = transaction
                .query_row(
                    "SELECT name, coins FROM profiles WHERE id = ?1",
                    [&credit.profile.0],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?
                .ok_or(StoreError::UnknownProfile)?;
            let inserted = transaction.execute(
                "INSERT OR IGNORE INTO credited_runs (run_id, profile_id) VALUES (?1, ?2)",
                [&credit.run.0, &credit.profile.0],
            )?;
            let balance = if inserted == 1 {
                let total = balance
                    .checked_add(coins)
                    .ok_or_else(|| StoreError::Invalid("coin balance overflow".into()))?;
                transaction.execute(
                    "UPDATE profiles SET coins = ?2 WHERE id = ?1",
                    params![credit.profile.0, total],
                )?;
                total
            } else {
                balance
            };
            transaction.commit()?;
            profile(credit.profile.0, name, balance)
        })
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(ErrorCode::NotADatabase) => Self::Invalid("not a database".into()),
            _ => Self::Unavailable(error.to_string()),
        }
    }
}

/// A fresh directory per test, removed on drop.
#[cfg(test)]
pub struct TempDir(pub PathBuf);

#[cfg(test)]
impl TempDir {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "car-drive-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn file(&self) -> PathBuf {
        self.0.join(FILE_NAME)
    }
}

#[cfg(test)]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::RunId;
    use bevy::tasks::block_on;

    fn name(name: &str) -> PlayerName {
        PlayerName::parse(name).unwrap()
    }

    fn credit(profile: &Profile, run: &str, coins: u64) -> RunCredit {
        RunCredit {
            run: RunId(run.into()),
            profile: profile.id.clone(),
            coins,
            proof: None,
        }
    }

    #[test]
    fn creates_and_reloads_a_profile_in_a_new_directory() {
        let dir = TempDir::new();
        let path = dir.0.join("nested").join(FILE_NAME);
        let store = SqliteStore::new(Some(path.clone()));
        assert_eq!(block_on(store.load()), Ok(None));
        let created = block_on(store.create(name("Ada"))).unwrap();
        assert_eq!(created.coins, 0);
        let reloaded = block_on(SqliteStore::new(Some(path)).load()).unwrap();
        assert_eq!(reloaded, Some(created));
    }

    #[test]
    fn credits_are_idempotent_per_run_and_persist() {
        let dir = TempDir::new();
        let store = SqliteStore::new(Some(dir.file()));
        let ada = block_on(store.create(name("Ada"))).unwrap();
        assert_eq!(
            block_on(store.credit(credit(&ada, "a", 12))).unwrap().coins,
            12
        );
        assert_eq!(
            block_on(store.credit(credit(&ada, "a", 12))).unwrap().coins,
            12
        );
        assert_eq!(
            block_on(store.credit(credit(&ada, "b", 5))).unwrap().coins,
            17
        );
        let reloaded = block_on(SqliteStore::new(Some(dir.file())).load()).unwrap();
        assert_eq!(reloaded.unwrap().coins, 17);
    }

    #[test]
    fn a_failed_credit_records_nothing() {
        let dir = TempDir::new();
        let store = SqliteStore::new(Some(dir.file()));
        let ada = block_on(store.create(name("Ada"))).unwrap();
        let stranger = Profile {
            id: ProfileId("missing".into()),
            ..ada.clone()
        };
        assert_eq!(
            block_on(store.credit(credit(&stranger, "a", 9))),
            Err(StoreError::UnknownProfile)
        );
        // The run id was not consumed by the failed attempt.
        assert_eq!(
            block_on(store.credit(credit(&ada, "a", 9))).unwrap().coins,
            9
        );
    }

    #[test]
    fn without_a_path_storage_is_unavailable() {
        let store = SqliteStore::new(None);
        assert!(matches!(
            block_on(store.load()),
            Err(StoreError::Unavailable(_))
        ));
    }

    #[test]
    fn an_empty_file_is_initialised() {
        let dir = TempDir::new();
        std::fs::write(dir.file(), b"").unwrap();
        let store = SqliteStore::new(Some(dir.file()));
        assert_eq!(block_on(store.load()), Ok(None));
        assert!(block_on(store.create(name("Ada"))).is_ok());
    }

    #[test]
    fn local_and_authoritative_market_files_cannot_be_interchanged() {
        let dir = TempDir::new();
        let local = SqliteStore::new(Some(dir.file()));
        block_on(local.create(name("Local"))).unwrap();
        assert!(block_on(SqliteStore::new_market(dir.file()).load()).is_err());
        let path = dir.0.join("market.sqlite3");
        let market = SqliteStore::new_market(path.clone());
        block_on(market.provision(name("Online"), 0)).unwrap();
        assert!(block_on(SqliteStore::new(Some(path)).load()).is_err());
    }

    #[test]
    fn foreign_or_newer_files_are_refused_unchanged() {
        let dir = TempDir::new();
        let newer = dir.0.join("newer.sqlite3");
        let connection = Connection::open(&newer).unwrap();
        connection.execute_batch("CREATE TABLE t (x)").unwrap();
        connection
            .pragma_update(None, "application_id", APPLICATION_ID)
            .unwrap();
        connection.pragma_update(None, "user_version", 3).unwrap();
        drop(connection);
        let foreign = dir.0.join("foreign.sqlite3");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        let garbage = dir.0.join("garbage.sqlite3");
        std::fs::write(&garbage, vec![0x5a; 4096]).unwrap();

        let cases = [
            (
                newer,
                StoreError::NewerSchema {
                    found: 3,
                    supported: 2,
                },
            ),
            (foreign, StoreError::Invalid("not a Car Drive save".into())),
            (garbage, StoreError::Invalid("not a database".into())),
        ];
        for (path, expected) in cases {
            let before = std::fs::read(&path).unwrap();
            let store = SqliteStore::new(Some(path.clone()));
            assert_eq!(block_on(store.load()), Err(expected.clone()));
            assert_eq!(block_on(store.create(name("Ada"))), Err(expected));
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn edited_invalid_data_is_rejected_on_load() {
        let dir = TempDir::new();
        let store = SqliteStore::new(Some(dir.file()));
        block_on(store.create(name("Ada"))).unwrap();
        Connection::open(dir.file())
            .unwrap()
            .execute("UPDATE profiles SET name = ' '", [])
            .unwrap();
        assert!(matches!(
            block_on(SqliteStore::new(Some(dir.file())).load()),
            Err(StoreError::Invalid(_))
        ));
    }
}
