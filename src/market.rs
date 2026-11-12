//! Authenticated private test market over bounded, length-prefixed JSON.
//! Bind/connect only to loopback; use an SSH tunnel for a second machine.
use crate::drive_proof::{DriveProof, DriveTicket};
use crate::storage::sqlite::SqliteStore;
use crate::storage::{
    PlayerName, Profile, ProfileId, ProfileStore, RunCredit, RunId, StoreError, StoreResult,
};
use crate::vehicles::{GarageAction, GarageRequest, GarageSnapshot};
use bevy::tasks::{BoxedFuture, IoTaskPool, block_on};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

const PROTOCOL: u32 = 1;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_ADDRESS: &str = "127.0.0.1:47831";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    address: SocketAddr,
    token: String,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    token: String,
    client: String,
    command: Command,
}
#[derive(Serialize, Deserialize)]
enum Command {
    Login,
    Garage(GarageRequest),
    Begin(String),
    Finish(DriveProof),
}
#[derive(Serialize, Deserialize)]
enum Reply {
    Profile(Profile),
    Garage(GarageSnapshot),
    Ticket(DriveTicket),
}

fn unavailable(e: impl ToString) -> StoreError {
    StoreError::Unavailable(e.to_string())
}
fn rejected(message: &str) -> StoreError {
    StoreError::Rejected(message.into())
}

fn write_frame(stream: &mut impl Write, value: &impl Serialize) -> StoreResult<()> {
    let bytes = serde_json::to_vec(value).map_err(unavailable)?;
    if bytes.len() > MAX_BYTES {
        return Err(rejected("Message too large"));
    }
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .map_err(unavailable)?;
    stream.write_all(&bytes).map_err(unavailable)
}
fn read_frame<T: DeserializeOwned>(stream: &mut impl Read) -> StoreResult<T> {
    let mut length = [0; 4];
    stream.read_exact(&mut length).map_err(unavailable)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_BYTES {
        return Err(rejected("Invalid message length"));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).map_err(unavailable)?;
    serde_json::from_slice(&bytes).map_err(|_| rejected("Malformed message"))
}

#[derive(Clone)]
pub struct NetworkStore {
    address: SocketAddr,
    token: String,
    client: String,
}
impl NetworkStore {
    pub fn from_file(path: &std::path::Path) -> StoreResult<Self> {
        let bytes = std::fs::read(path).map_err(unavailable)?;
        if bytes.len() > 4096 {
            return Err(rejected("Credential file too large"));
        }
        let c: Credentials = serde_json::from_slice(&bytes).map_err(unavailable)?;
        if !c.address.ip().is_loopback() || c.token.len() != 64 {
            return Err(rejected(
                "Use a loopback address or an SSH tunnel and a provisioned token",
            ));
        }
        Ok(Self {
            address: c.address,
            token: c.token,
            client: RunId::random().0,
        })
    }
    fn request(&self, command: Command) -> BoxedFuture<'static, StoreResult<Reply>> {
        let address = self.address;
        let request = Envelope {
            version: PROTOCOL,
            token: self.token.clone(),
            client: self.client.clone(),
            command,
        };
        let work = async move {
            let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))
                .map_err(unavailable)?;
            stream
                .set_read_timeout(Some(Duration::from_secs(60)))
                .map_err(unavailable)?;
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .map_err(unavailable)?;
            write_frame(&mut stream, &request)?;
            read_frame::<StoreResult<Reply>>(&mut stream)?
        };
        match IoTaskPool::try_get() {
            Some(pool) => Box::pin(pool.spawn(work)),
            None => Box::pin(work),
        }
    }
}
impl ProfileStore for NetworkStore {
    fn online(&self) -> bool {
        true
    }
    fn load(&self) -> BoxedFuture<'static, StoreResult<Option<Profile>>> {
        let future = self.request(Command::Login);
        Box::pin(async move {
            match future.await? {
                Reply::Profile(p) => Ok(Some(p)),
                _ => Err(rejected("Unexpected market reply")),
            }
        })
    }
    fn create(&self, _name: PlayerName) -> BoxedFuture<'static, StoreResult<Profile>> {
        Box::pin(async { Err(rejected("Ask the test market operator for an account")) })
    }
    fn credit(&self, credit: RunCredit) -> BoxedFuture<'static, StoreResult<Profile>> {
        let Some(proof) = credit.proof else {
            return Box::pin(async { Err(rejected("Online coins require a verified drive")) });
        };
        let future = self.request(Command::Finish(proof));
        Box::pin(async move {
            match future.await? {
                Reply::Profile(p) => Ok(p),
                _ => Err(rejected("Unexpected market reply")),
            }
        })
    }
    fn garage(
        &self,
        _profile: ProfileId,
        request: GarageRequest,
    ) -> BoxedFuture<'static, StoreResult<GarageSnapshot>> {
        let future = self.request(Command::Garage(request));
        Box::pin(async move {
            match future.await? {
                Reply::Garage(g) => Ok(g),
                _ => Err(rejected("Unexpected market reply")),
            }
        })
    }
    fn begin_drive(&self, id: String) -> BoxedFuture<'static, StoreResult<Option<DriveTicket>>> {
        let future = self.request(Command::Begin(id));
        Box::pin(async move {
            match future.await? {
                Reply::Ticket(t) => Ok(Some(t)),
                _ => Err(rejected("Unexpected market reply")),
            }
        })
    }
}

struct Service {
    store: SqliteStore,
}
impl Service {
    /// Serialized by the service mutex: authentication, idle checks and a write
    /// form one request boundary, including a concurrent login from another client.
    fn handle(&mut self, envelope: Envelope) -> StoreResult<Reply> {
        if envelope.version != PROTOCOL {
            return Err(rejected("Incompatible game/protocol version"));
        }
        let profile = block_on(self.store.authenticate(
            envelope.token,
            envelope.client,
            matches!(envelope.command, Command::Login),
        ))?;
        match envelope.command {
            Command::Login => Ok(Reply::Profile(profile)),
            Command::Garage(request) => {
                if !matches!(request.action, GarageAction::Refresh { .. }) {
                    block_on(self.store.market_idle(profile.id.clone()))?;
                }
                block_on(self.store.garage(profile.id, request)).map(Reply::Garage)
            }
            Command::Begin(id) => {
                block_on(self.store.issue_drive(profile.id, id)).map(Reply::Ticket)
            }
            Command::Finish(proof) => {
                block_on(self.store.finish_drive(profile.id, proof)).map(Reply::Profile)
            }
        }
    }
}

fn serve_connection(mut stream: TcpStream, service: &Mutex<Service>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let result = read_frame::<Envelope>(&mut stream).and_then(|request| {
        service
            .lock()
            .map_err(|_| unavailable("Market service lock failed"))?
            .handle(request)
    });
    let _ = write_frame(&mut stream, &result);
}

fn serve(path: PathBuf, address: SocketAddr) -> StoreResult<()> {
    if !address.ip().is_loopback() {
        return Err(rejected(
            "Test market binds only to loopback. Use SSH forwarding for remote clients.",
        ));
    }
    let store = SqliteStore::new_market(path);
    block_on(store.load())?;
    let listener = TcpListener::bind(address).map_err(unavailable)?;
    let service = Arc::new(Mutex::new(Service { store }));
    // Bound connections and threads. Read requests outside the transaction lock.
    let (send, receive) = std::sync::mpsc::sync_channel::<TcpStream>(8);
    let receive = Arc::new(Mutex::new(receive));
    for _ in 0..4 {
        let receive = receive.clone();
        let service = service.clone();
        std::thread::spawn(move || {
            loop {
                let stream = match receive.lock() {
                    Ok(r) => r.recv(),
                    Err(_) => return,
                };
                match stream {
                    Ok(stream) => serve_connection(stream, &service),
                    Err(_) => return,
                }
            }
        });
    }
    println!("Car Drive private test market listening on {address}");
    for stream in listener.incoming() {
        send.try_send(stream.map_err(unavailable)?).ok();
    }
    Ok(())
}

/// Operator commands run before creating any graphical app.
pub fn command_line(args: &[String]) -> Option<StoreResult<()>> {
    match args.get(1).map(String::as_str) {
        Some("--market-server") => Some((|| {
            let path = args
                .get(2)
                .ok_or_else(|| rejected("Usage: --market-server DATABASE [127.0.0.1:47831]"))?;
            let address = args
                .get(3)
                .map_or(DEFAULT_ADDRESS, String::as_str)
                .parse()
                .map_err(unavailable)?;
            serve(path.into(), address)
        })()),
        Some("--market-provision") => Some((|| {
            if args.len() < 5 || args.len() > 6 {
                return Err(rejected(
                    "Usage: --market-provision DATABASE NAME CREDENTIAL_FILE [TEST_COINS]",
                ));
            }
            let name = PlayerName::parse(&args[3]).map_err(unavailable)?;
            let coins = args
                .get(5)
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(unavailable)?
                .unwrap_or(0);
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut output = options.open(&args[4]).map_err(unavailable)?;
            let store = SqliteStore::new_market(args[2].clone().into());
            let token = block_on(store.provision(name, coins))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&args[2], std::fs::Permissions::from_mode(0o600))
                    .map_err(unavailable)?;
            }
            let credentials = Credentials {
                address: DEFAULT_ADDRESS.parse().map_err(unavailable)?,
                token,
            };
            output
                .write_all(&serde_json::to_vec_pretty(&credentials).map_err(unavailable)?)
                .map_err(unavailable)?;
            output.sync_all().map_err(unavailable)?;
            println!(
                "Provisioned a private test account; credentials saved to {}",
                args[4]
            );
            Ok(())
        })()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::TempDir;
    use crate::vehicles::{Change, Model, Paint, Upgrade};

    #[test]
    fn bounded_frames_reject_oversized_or_invalid_input() {
        assert!(
            read_frame::<Envelope>(&mut ((MAX_BYTES as u32 + 1).to_be_bytes().as_slice())).is_err()
        );
        let mut bytes = vec![];
        write_frame(&mut bytes, &"wrong type").unwrap();
        assert!(read_frame::<Envelope>(&mut bytes.as_slice()).is_err());
    }

    #[test]
    fn service_credits_only_verified_drives_once_and_locks_the_garage() {
        let dir = TempDir::new();
        let store = SqliteStore::new_market(dir.file());
        let token = block_on(store.provision(PlayerName::parse("Ada").unwrap(), 100)).unwrap();
        let mut service = Service {
            store: store.clone(),
        };
        let client = RunId::random().0;
        let envelope = |command| Envelope {
            version: PROTOCOL,
            token: token.clone(),
            client: client.clone(),
            command,
        };
        service.handle(envelope(Command::Login)).unwrap();
        let id = RunId::random().0;
        let Reply::Ticket(ticket) = service
            .handle(envelope(Command::Begin(id.clone())))
            .unwrap()
        else {
            panic!()
        };
        let Reply::Ticket(retry) = service.handle(envelope(Command::Begin(id))).unwrap() else {
            panic!()
        };
        assert_eq!(retry.seed, ticket.seed);
        assert!(
            service
                .handle(envelope(Command::Garage(GarageRequest::new(
                    GarageAction::Buy(Model::BobbyCar)
                ))))
                .is_err()
        );
        let forged = DriveProof {
            ticket: ticket.id.clone(),
            frames: vec![crate::drive_proof::Frame(0, false); 6400],
        };
        assert!(service.handle(envelope(Command::Finish(forged))).is_err());
        // Tests own the recorded start time; no sleep or trust in a client clock.
        block_on(store.run(|conn| {
            conn.execute(
                "UPDATE market_drives SET started = (unixepoch() - 10) * 1000",
                [],
            )?;
            Ok(())
        }))
        .unwrap();
        let proof = DriveProof {
            ticket: ticket.id,
            frames: vec![crate::drive_proof::Frame(0, false); 128],
        };
        let Reply::Profile(after) = service
            .handle(envelope(Command::Finish(proof.clone())))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(after.coins, 106);
        let Reply::Profile(repeated) = service
            .handle(envelope(Command::Finish(proof.clone())))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(repeated, after);
        let mut altered = proof;
        altered.frames.pop();
        assert!(service.handle(envelope(Command::Finish(altered))).is_err());
        assert!(
            service
                .handle(envelope(Command::Garage(GarageRequest::new(
                    GarageAction::Buy(Model::BobbyCar)
                ))))
                .is_ok()
        );
    }

    #[test]
    fn two_authenticated_clients_trade_over_tcp_and_reload() {
        let dir = TempDir::new();
        let store = SqliteStore::new_market(dir.file());
        let token_a =
            block_on(store.provision(PlayerName::parse("Ada").unwrap(), 200_000)).unwrap();
        let token_b =
            block_on(store.provision(PlayerName::parse("Bob").unwrap(), 200_000)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let service = Arc::new(Mutex::new(Service { store }));
        let worker_service = service.clone();
        // Exact request count avoids sleeps or a leaked background test server.
        let worker = std::thread::spawn(move || {
            for stream in listener.incoming().take(13) {
                serve_connection(stream.unwrap(), &worker_service);
            }
        });
        let a = NetworkStore {
            address,
            token: token_a,
            client: RunId::random().0,
        };
        let b = NetworkStore {
            address,
            token: token_b,
            client: RunId::random().0,
        };
        let pa = block_on(a.load()).unwrap().unwrap();
        let pb = block_on(b.load()).unwrap().unwrap();
        let action = |client: &NetworkStore, id: &ProfileId, action| {
            block_on(client.garage(id.clone(), GarageRequest::new(action))).unwrap()
        };
        let bought = action(&a, &pa.id, GarageAction::Buy(Model::Rs6));
        let car = bought
            .vehicles
            .iter()
            .find(|v| v.model == Model::Rs6)
            .unwrap()
            .id
            .clone();
        action(
            &a,
            &pa.id,
            GarageAction::Configure {
                vehicle: car.clone(),
                change: Change::Paint(Paint::Green),
            },
        );
        action(
            &a,
            &pa.id,
            GarageAction::Configure {
                vehicle: car.clone(),
                change: Change::Buy(Upgrade::Motor1),
            },
        );
        let sell = GarageRequest::new(GarageAction::Sell(car.clone()));
        let sold = block_on(a.garage(pa.id.clone(), sell.clone())).unwrap();
        assert_eq!(block_on(a.garage(pa.id.clone(), sell)).unwrap(), sold);
        let acquired = action(&b, &pb.id, GarageAction::BuyListing(car.clone()));
        let transferred = acquired.vehicles.iter().find(|v| v.id == car).unwrap();
        assert_eq!((transferred.paint, transferred.motor), (Paint::Green, 1));
        assert_eq!(block_on(a.load()).unwrap().unwrap().coins, sold.coins);
        assert_eq!(block_on(b.load()).unwrap().unwrap().coins, acquired.coins);
        let wrong = NetworkStore {
            token: "0".repeat(64),
            ..a.clone()
        };
        assert!(block_on(wrong.load()).is_err());
        assert!(
            block_on(b.garage(
                pb.id.clone(),
                GarageRequest::new(GarageAction::Sell(pa.id.0.clone()))
            ))
            .is_err()
        );
        let reconnected = NetworkStore {
            client: RunId::random().0,
            ..b
        };
        assert_eq!(
            block_on(reconnected.load()).unwrap().unwrap().coins,
            acquired.coins
        );
        worker.join().unwrap();
    }
}
