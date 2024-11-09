#![cfg_attr(feature = "strict", deny(warnings))]

use async_broadcast::broadcast;
use async_std::task;
use shared::clap::Parser;
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::log;
use shared::prost::Message;
use shared::simple_logger;
use shared::{clap, nats};
use std::net::TcpListener;
use tungstenite::accept;

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

#[async_std::main]
async fn main() {
    let args = Args::parse();
    simple_logger::init_with_level(args.log_level).unwrap();

    let (mut sender, broadcast_receiver) = broadcast(128);
    sender.set_overflow(true);
    let inactive_broadcast_receiver = broadcast_receiver.deactivate();

    // nano message receive task
    task::spawn(async move {
        let nc =
            nats::connect(args.nats_address).expect("should be able to connect to NATS server");
        let sub = nc.subscribe("*").expect("could not subscribe to topic '*'");
        for msg in sub.messages() {
            let unwrapped = event_msg::EventMsg::decode(msg.data.as_slice())
                .unwrap()
                .event;
            if let Some(event) = unwrapped {
                if let Err(e) = sender.broadcast(event).await {
                    log::error!("Could not send msg event into broadcast channel: {}", e);
                }
            }
        }
    });

    log::info!("Starting websocket server on {}", args.websocket_address);
    let server = match TcpListener::bind(args.websocket_address.clone()) {
        Ok(s) => s,
        Err(e) => {
            log::error!(
                "Could not start websocket server on {}: {}",
                args.websocket_address,
                e
            );
            return;
        }
    };

    for stream in server.incoming() {
        match stream {
            Ok(stream) => {
                let mut r = inactive_broadcast_receiver.clone().activate();
                task::spawn(async move {
                    match accept(stream) {
                        Ok(mut websocket) => {
                            log::info!(
                                "Accepted new websocket connection: connections={}",
                                r.receiver_count()
                            );

                            loop {
                                match r.recv().await {
                                    Ok(msg) => {
                                        match serde_json::to_string::<Event>(&msg.clone().into()) {
                                            Ok(msg) => {
                                                if let Err(e) =
                                                    websocket.send(tungstenite::Message::Text(msg))
                                                {
                                                    log::warn!("Could not send msg to websocket: {}. Connection probably closed.", e);
                                                    // Try our best to close and flush the websocket. If we can't,
                                                    // we can't..
                                                    let _ = websocket.close(None);
                                                    let _ = websocket.flush();
                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                log::error!(
                                                    "Could not serialize the message to JSON: {}",
                                                    e
                                                )
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Could not receive msg: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to open websocket on incoming connection: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                log::warn!("Failed to accept incomming TCP connection: {}", e);
            }
        }
    }
}
