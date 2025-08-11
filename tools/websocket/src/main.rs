#![cfg_attr(feature = "strict", deny(warnings))]

use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::log;
use shared::prost::Message;
use shared::simple_logger;
use shared::{clap, nats};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use tungstenite::{accept, Message as TungsteniteMessage, WebSocket};

/// A peer-observer tool that sends out all events on a websocket
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The NATS server address the tool should connect and subscribe to.
    #[arg(short, long, default_value = "127.0.0.1:4222")]
    nats_address: String,

    /// The websocket address the tool listens on.
    #[arg(short, long, default_value = "127.0.0.1:47482")]
    websocket_address: String,

    /// The log level the took should run with. Valid log levels are "trace",
    /// "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html
    #[arg(short, long, default_value_t = log::Level::Debug)]
    log_level: log::Level,
}

fn main() {
    let args = Args::parse();
    simple_logger::init_with_level(args.log_level).unwrap();

    log::info!("Trying to connect to NATS server at {}", args.nats_address);
    let nc = nats::connect(args.nats_address).expect("should be able to connect to NATS server");
    let sub = nc.subscribe("*").expect("could not subscribe to topic '*'");

    let clients = Arc::new(Mutex::new(Vec::new()));

    // Spawn a thread to handle NATS messages and broadcast to WebSocket clients
    {
        let clients = Arc::clone(&clients);
        thread::spawn(move || {
            for msg in sub.messages() {
                let unwrapped = event_msg::EventMsg::decode(msg.data.as_slice())
                    .unwrap()
                    .event;
                if let Some(event) = unwrapped {
                    match serde_json::to_string::<Event>(&event.clone().into()) {
                        Ok(msg) => {
                            broadcast_to_clients(&msg, &clients);
                        }
                        Err(e) => {
                            log::error!("Could not serialize the message to JSON: {}", e)
                        }
                    }
                }
            }
        });
    }

    log::info!("Starting websocket server on {}", args.websocket_address);
    let server = TcpListener::bind(args.websocket_address).expect("Failed to bind server");

    // Accept WebSocket clients
    for stream in server.incoming() {
        match stream {
            Ok(stream) => {
                let clients = Arc::clone(&clients);
                thread::spawn(move || {
                    handle_client(stream, clients);
                });
            }
            Err(e) => log::warn!("Failed to accept connection: {}", e),
        }
    }
}

fn handle_client(stream: TcpStream, clients: Arc<Mutex<Vec<TcpStream>>>) {
    let mut websocket = accept(stream).expect("Failed to accept WebSocket");
    {
        let mut clients_guard = clients.lock().unwrap();
        clients_guard.push(websocket.get_ref().try_clone().unwrap());
    }

    log::info!(
        "Client '{}' connected",
        websocket.get_ref().peer_addr().unwrap()
    );
    loop {
        match websocket.read() {
            Ok(m) => {
                match m {
                    TungsteniteMessage::Close(_) => {
                        // Remove the client from the shared list if the connection is closed
                        let mut clients_guard = clients.lock().unwrap();
                        clients_guard.retain(|client| {
                            client.peer_addr().ok().map_or(false, |addr| {
                                addr != websocket.get_ref().peer_addr().unwrap()
                            })
                        });
                        break;
                    }
                    _ => (), // We ignore all other messages a client sends us.
                }
            }
            Err(_) => {
                log::info!(
                    "Client '{}' disconnected",
                    websocket.get_ref().peer_addr().unwrap()
                );
                // Remove the client from the shared list if the connection is closed
                let mut clients_guard = clients.lock().unwrap();
                clients_guard.retain(|client| {
                    client.peer_addr().ok().map_or(false, |addr| {
                        addr != websocket.get_ref().peer_addr().unwrap()
                    })
                });
                break;
            }
        }
    }
}

fn broadcast_to_clients(message: &str, clients: &Arc<Mutex<Vec<TcpStream>>>) {
    let clients_guard = clients.lock().unwrap();

    for client in clients_guard.iter() {
        let mut websocket = WebSocket::from_raw_socket(
            client.try_clone().unwrap(),
            tungstenite::protocol::Role::Server,
            None,
        );

        if let Err(e) = websocket.send(TungsteniteMessage::text(message)) {
            log::warn!("Failed to send message to a client: {}", e);
        }
    }
}
