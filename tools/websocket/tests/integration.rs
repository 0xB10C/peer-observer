#![cfg(feature = "nats_integration_tests")]

use shared::{
    event_msg::{event_msg::Event, EventMsg},
    futures::StreamExt,
    log,
    nats_publisher_for_testing::NatsPublisherForTesting,
    nats_server_for_testing::NatsServerForTesting,
    nats_subjects::Subject,
    net_conn::{self, Connection},
    net_msg::{self, message::Msg, Metadata, Ping, Pong},
    prost::Message,
    rand::{self, Rng},
    simple_logger::SimpleLogger,
    tokio::{self, sync::watch, time::sleep},
};

use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        Once, OnceLock,
    },
    time::Duration,
};

use websocket::Args;

static INIT: Once = Once::new();

static NEXT_NATS_PORT: OnceLock<AtomicU16> = OnceLock::new();
static NEXT_WEBSOCKET_PORT: OnceLock<AtomicU16> = OnceLock::new();

fn setup() -> (u16, u16) {
    INIT.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Trace)
            .init()
            .unwrap();

        let mut rng = rand::rng();

        // choose start ports from the ephemeral port range
        let nats_start = rng.random_range(49152..65500);
        let websocket_start = rng.random_range(49152..65500);

        NEXT_NATS_PORT.set(AtomicU16::new(nats_start)).unwrap();
        NEXT_WEBSOCKET_PORT
            .set(AtomicU16::new(websocket_start))
            .unwrap();
    });

    let nats_port = NEXT_NATS_PORT.get().unwrap().fetch_add(1, Ordering::SeqCst);
    let websocket_port = NEXT_WEBSOCKET_PORT
        .get()
        .unwrap()
        .fetch_add(1, Ordering::SeqCst);
    return (nats_port, websocket_port);
}

fn make_test_args(nats_port: u16, websocket_port: u16) -> Args {
    Args::new(
        format!("127.0.0.1:{}", nats_port),
        format!("127.0.0.1:{}", websocket_port),
        log::Level::Trace,
    )
}

async fn publish_and_check(
    events: &[EventMsg],
    subject: Subject,
    expected: &[&str],
    num_clients: u8,
) {
    let (nats_port, websocket_port) = setup();
    let _nats_server = NatsServerForTesting::new(nats_port).await;
    let nats_publisher = NatsPublisherForTesting::new(nats_port).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let args = make_test_args(nats_port, websocket_port);
    let websocket_handle = tokio::spawn(async move {
        websocket::run(args, shutdown_rx).await.unwrap();
    });
    // allow the websocket tool to start
    sleep(Duration::from_secs(1)).await;

    let mut clients = vec![];
    for _ in 0..num_clients {
        let (ws_stream, _) =
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}", websocket_port))
                .await
                .expect("Should be able to connect to websocket");
        clients.push(ws_stream)
    }

    for event in events.iter() {
        println!("publishing: {:?}", event);
        nats_publisher
            .publish(subject.to_string(), event.encode_to_vec())
            .await;
    }

    sleep(Duration::from_millis(100)).await;

    assert_eq!(events.len(), expected.len());
    for mut client in clients {
        for expected in expected.iter() {
            if let Some(msg) = client.next().await {
                let msg = msg.unwrap();
                println!("data: {}", msg.to_string());
                assert_eq!(*expected, msg.to_string());
            }
        }
    }

    shutdown_tx.send(true).unwrap();
    websocket_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_websocket_conn_inbound() {
    println!("test that inbound connections work");

    publish_and_check(&[EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
        event: Some(net_conn::connection_event::Event::Inbound(
            shared::net_conn::InboundConnection {
                conn: Connection {
                    addr: "127.0.0.1:8333".to_string(),
                    conn_type: 1,
                    network: 2,
                    peer_id: 7,
                },
                existing_connections: 123,
            },
        )),
    }))
    .unwrap()], Subject::NetConn, &vec![
        r#"{"Conn":{"event":{"Inbound":{"conn":{"peer_id":7,"addr":"127.0.0.1:8333","conn_type":1,"network":2},"existing_connections":123}}}}"#,
    ],1).await;
}

#[tokio::test]
async fn test_integration_websocket_p2p_message_ping() {
    println!("test that the P2P message via websockets work");

    publish_and_check(
        &[
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 0,
                    addr: "127.0.0.1:8333".to_string(),
                    conn_type: 1,
                    command: "ping".to_string(),
                    inbound: true,
                    size: 8,
                },
                msg: Some(Msg::Ping(Ping { value: 1 })),
            }))
            .unwrap(),
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 0,
                    addr: "127.0.0.1:8333".to_string(),
                    conn_type: 1,
                    command: "pong".to_string(),
                    inbound: false,
                    size: 8,
                },
                msg: Some(Msg::Pong(Pong { value: 1 })),
            }))
            .unwrap(),
        ],
        Subject::NetMsg,
        &vec![
            r#"{"Msg":{"meta":{"peer_id":0,"addr":"127.0.0.1:8333","conn_type":1,"command":"ping","inbound":true,"size":8},"msg":{"Ping":{"value":1}}}}"#,
            r#"{"Msg":{"meta":{"peer_id":0,"addr":"127.0.0.1:8333","conn_type":1,"command":"pong","inbound":false,"size":8},"msg":{"Pong":{"value":1}}}}"#,
        ],
        1
    )
    .await;
}

#[tokio::test]
async fn test_integration_websocket_multi_client() {
    println!("test that multiple clients all receive the messages");

    publish_and_check(
        &[
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 0,
                    addr: "127.0.0.1:8333".to_string(),
                    conn_type: 1,
                    command: "ping".to_string(),
                    inbound: true,
                    size: 8,
                },
                msg: Some(Msg::Ping(Ping { value: 1 })),
            }))
            .unwrap(),
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 0,
                    addr: "127.0.0.1:8333".to_string(),
                    conn_type: 1,
                    command: "pong".to_string(),
                    inbound: false,
                    size: 8,
                },
                msg: Some(Msg::Pong(Pong { value: 1 })),
            }))
            .unwrap(),
        ],
        Subject::NetMsg,
        &vec![
            r#"{"Msg":{"meta":{"peer_id":0,"addr":"127.0.0.1:8333","conn_type":1,"command":"ping","inbound":true,"size":8},"msg":{"Ping":{"value":1}}}}"#,
            r#"{"Msg":{"meta":{"peer_id":0,"addr":"127.0.0.1:8333","conn_type":1,"command":"pong","inbound":false,"size":8},"msg":{"Pong":{"value":1}}}}"#,
        ],
        12
    )
    .await;
}
