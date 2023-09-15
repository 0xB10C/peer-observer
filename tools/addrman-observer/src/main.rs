#![cfg_attr(feature = "strict", deny(warnings))]

use env_logger::Env;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use shared::util;
use shared::wrapper;
use shared::wrapper::wrapper::Wrap;

use futures_util::{SinkExt, StreamExt, TryFutureExt};
use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};
use prost::Message;
use serde::Serialize;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message as WSMessage, WebSocket};
use warp::Filter;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

const ADDRMAN_NEW_BUCKETS: usize = 1024;
const ADDRMAN_TRIED_BUCKETS: usize = 256;
const ADDRMAN_BUCKET_ENTRIES: usize = 64;

// global client id counter.
static NEXT_CLIENT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Serialize, Debug)]
struct AddrInfo {
    pub addr: String,
    pub addr_subnet: String,
    pub source: String,
    pub source_subnet: String,
    pub bucket: i32,
    pub bucket_pos: i32,
    pub time_added: u64,
    pub new_table: bool,
}

// Our state of currently connected clients.
// - Key is their id
// - Value is a sender of `warp::ws::Message`
type Clients = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<WSMessage>>>>;
type Table = Arc<RwLock<Vec<Option<AddrInfo>>>>;

const INIT: Option<AddrInfo> = None;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let index_path = env::args().nth(1).expect("No path to index.html provided.");
    let address = env::args()
        .nth(2)
        .expect("No web server address to bind on provided (e.g. 'localhost:3030')");

    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    let clients = Clients::default();
    let new_table = Arc::new(RwLock::new(vec![
        INIT;
        ADDRMAN_NEW_BUCKETS
            * ADDRMAN_BUCKET_ENTRIES
    ]));
    let tried_table = Arc::new(RwLock::new(vec![
        INIT;
        ADDRMAN_TRIED_BUCKETS
            * ADDRMAN_BUCKET_ENTRIES
    ]));

    let clients_clone = clients.clone();
    let new_table_clone = new_table.clone();
    let tried_table_clone = tried_table.clone();

    tokio::task::spawn(async move {
        loop {
            let msg = sub.recv().unwrap();
            let unwrapped = wrapper::Wrapper::decode(msg.as_slice()).unwrap().wrap;
            if let Some(event) = unwrapped {
                match event {
                    Wrap::Addrman(aevent) => match aevent.event.unwrap() {
                        shared::addrman::addrman_event::Event::New(new) => {
                            if !new.inserted {
                                continue;
                            }
                            let info = AddrInfo {
                                addr: new.addr.clone(),
                                addr_subnet: util::subnet(util::ip_from_ipport(new.addr)),
                                source: new.source.clone(),
                                source_subnet: util::subnet(new.source),
                                bucket: new.bucket,
                                bucket_pos: new.bucket_pos,
                                time_added: 0,
                                new_table: true,
                            };
                            log::debug!("entry in new table: {:?}", info);
                            new_table_clone.write().await[new.bucket as usize
                                * ADDRMAN_BUCKET_ENTRIES
                                + new.bucket_pos as usize] = Some(info.clone());
                            let msg_content = serde_json::to_string(&vec![info]).unwrap();
                            let msg = WSMessage::text(msg_content);
                            for (&uid, tx) in clients_clone.read().await.iter() {
                                if let Err(_disconnected) = tx.send(msg.clone()) {
                                    log::warn!("client {} disconnected", uid);
                                }
                            }
                        }
                        shared::addrman::addrman_event::Event::Tried(tried) => {
                            let info = AddrInfo {
                                addr: tried.addr.clone(),
                                addr_subnet: util::subnet(util::ip_from_ipport(tried.addr)),
                                source: tried.source.clone(),
                                source_subnet: util::subnet(tried.source),
                                bucket: tried.bucket,
                                bucket_pos: tried.bucket_pos,
                                time_added: 0,
                                new_table: false,
                            };
                            tried_table_clone.write().await[tried.bucket as usize
                                * ADDRMAN_BUCKET_ENTRIES
                                + tried.bucket_pos as usize] = Some(info.clone());
                            let msg_content = serde_json::to_string(&vec![info]).unwrap();
                            let msg = WSMessage::text(msg_content);
                            for (&uid, tx) in clients_clone.read().await.iter() {
                                if let Err(_disconnected) = tx.send(msg.clone()) {
                                    log::warn!("client {} disconnected", uid);
                                }
                            }
                        }
                    },
                    _ => (),
                }
            }
        }
    });

    let clients_filter = warp::any().map(move || clients.clone());
    let new_table_filter = warp::any().map(move || new_table.clone());
    let tried_table_filter = warp::any().map(move || tried_table.clone());

    let addrman = warp::path("addrman")
        .and(warp::ws())
        .and(clients_filter)
        .and(new_table_filter)
        .and(tried_table_filter)
        .map(|ws: warp::ws::Ws, clients, new_table, tried_table| {
            ws.on_upgrade(move |socket| client_connected(socket, clients, new_table, tried_table))
        });

    let index = warp::get()
        .and(warp::path::end())
        .and(warp::fs::file(index_path));

    let routes = index.or(addrman);

    warp::serve(routes)
        .run(SocketAddr::from_str(&address).unwrap())
        .await;
}

async fn client_connected(ws: WebSocket, clients: Clients, new_table: Table, tried_table: Table) {
    let id = NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed);

    log::info!("new client: {}", id);

    // Split the socket into a sender and receive of messages.
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            ws_tx
                .send(message)
                .unwrap_or_else(|e| {
                    log::warn!("websocket send error: {}", e);
                })
                .await;
        }
    });

    // Save the sender in our list of connected clients
    clients.write().await.insert(id, tx.clone());

    // Send the current table contents as one message to the client.
    let contents: Vec<Option<AddrInfo>> = new_table
        .read()
        .await
        .iter()
        .chain(tried_table.read().await.iter())
        .filter(|info_option| info_option.is_some())
        .map(|info| info.clone())
        .collect();

    let msg_content = serde_json::to_string(&contents).unwrap();
    let msg = WSMessage::text(msg_content);
    if let Err(_disconnected) = tx.send(msg.clone()) {
        log::warn!("client {} disconnected", id);
    }

    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("websocket error(uid={}): {}", id, e);
                break;
            }
        };
    }

    log::info!("client disconnected: {}", id);
    clients.write().await.remove(&id);
}
