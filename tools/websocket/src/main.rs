#![cfg_attr(feature = "strict", deny(warnings))]

use async_broadcast::broadcast;
use async_std::task;
use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};
use shared::event_msg;
use shared::event_msg::event_msg::Event;
use shared::prost::Message;
use std::net::TcpListener;
use tungstenite::accept;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";
const WS_ADDRESS: &'static str = "127.0.0.1:47482";

#[async_std::main]
async fn main() {
    let (mut sender, broadcast_receiver) = broadcast(128);
    sender.set_overflow(true);
    let inactive_broadcast_receiver = broadcast_receiver.deactivate();

    // nano message receive task
    task::spawn(async move {
        let sub = Socket::new(Protocol::Sub0).unwrap();
        sub.dial(ADDRESS).unwrap();

        let all_topics = vec![];
        sub.set_opt::<Subscribe>(all_topics).unwrap();
        loop {
            let msg = sub.recv().unwrap();
            let unwrapped = event_msg::EventMsg::decode(msg.as_slice()).unwrap().event;
            if let Some(event) = unwrapped {
                if let Err(e) = sender.broadcast(event).await {
                    println!("Could not send msg event into broadcast channel: {}", e);
                }
            }
        }
    });

    println!("Starting websocket server on {}", WS_ADDRESS);
    let server = match TcpListener::bind(WS_ADDRESS) {
        Ok(s) => s,
        Err(e) => {
            println!("Could not start websocket server on {}: {}", WS_ADDRESS, e);
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
                            println!(
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
                                                    println!("Could not send msg to websocket: {}. Connection probably closed.", e);
                                                    // Try our best to close and flush the websocket. If we can't,
                                                    // we can't..
                                                    let _ = websocket.close(None);
                                                    let _ = websocket.flush();
                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                println!(
                                                    "Could not serialize the message to JSON: {}",
                                                    e
                                                )
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("Could not receive msg: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("Failed to open websocket on incoming connection: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                println!("Failed to accept incomming TCP connection: {}", e);
            }
        }
    }
}
