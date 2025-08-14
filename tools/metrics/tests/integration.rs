#![cfg(feature = "nats_integration_tests")]

use metrics::Args;

use shared::{
    event_msg::{event_msg::Event, EventMsg},
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
    io::{Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicU16, Ordering},
        Once, OnceLock,
    },
    time::Duration,
};

static INIT: Once = Once::new();

static NEXT_NATS_PORT: OnceLock<AtomicU16> = OnceLock::new();
static NEXT_METRICS_PORT: OnceLock<AtomicU16> = OnceLock::new();

fn setup() -> (u16, u16) {
    INIT.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Trace)
            .init()
            .unwrap();

        let mut rng = rand::rng();

        // choose start ports from the ephemeral port range
        let nats_start = rng.random_range(49152..65500);
        let metrics_start = rng.random_range(49152..65500);

        NEXT_NATS_PORT.set(AtomicU16::new(nats_start)).unwrap();
        NEXT_METRICS_PORT
            .set(AtomicU16::new(metrics_start))
            .unwrap();
    });

    let nats_port = NEXT_NATS_PORT.get().unwrap().fetch_add(1, Ordering::SeqCst);
    let metrics_port = NEXT_METRICS_PORT
        .get()
        .unwrap()
        .fetch_add(1, Ordering::SeqCst);
    return (nats_port, metrics_port);
}

fn make_test_args(nats_port: u16, metrics_port: u16) -> Args {
    Args::new(
        format!("127.0.0.1:{}", nats_port),
        format!("127.0.0.1:{}", metrics_port),
        log::Level::Trace,
    )
}

fn fetch_metrics(port: u16) -> Result<String, std::io::Error> {
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = TcpStream::connect(addr.clone())?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        addr
    );

    stream.write_all(request.as_bytes()).unwrap();
    stream.flush()?;

    // Read the full response until EOF (server closes the connection).
    let mut response = Vec::new();
    let mut s = stream;
    s.read_to_end(&mut response)?;

    Ok(String::from_utf8_lossy(&response).to_string())
}

fn check_metrics(port: u16, expected: &[&str]) -> Result<bool, std::io::Error> {
    let metrics_raw = fetch_metrics(port)?;

    println!("HTTP response from metrics server:\n");
    for line in metrics_raw.split("\n") {
        println!("{}", line);
    }

    let mut no_lines_missing = true;
    for line in expected {
        let line = line.trim();
        if line == "" {
            continue;
        }
        if !metrics_raw.contains(line) {
            println!("Response does not contain line: '{}'", line);
            no_lines_missing = false;
        }
    }
    return Ok(no_lines_missing);
}

#[tokio::test]
async fn test_integration_metrics_no_nats_connection() {
    println!("test that we fail if we can't connect to NATS (due to port 0)");
    let (_, _) = setup();

    let args = make_test_args(0, 0);
    let (_, shutdown_rx) = watch::channel(false);
    let result = metrics::run(args, shutdown_rx).await;

    assert!(result.is_err());
    // allows for easier debugging if it's not a Io error..
    if let Err(ref e) = result {
        println!("error: {}", e);
    }
    assert!(matches!(
        result,
        Err(metrics::error::RuntimeError::NatsConnect(_))
    ))
}

#[tokio::test]
async fn test_integration_metrics_basic() {
    println!("test that we can connect to NATS and query from the metrics server");
    let (nats_port, metrics_port) = setup();
    let _nats_server = NatsServerForTesting::new(nats_port).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let args = make_test_args(nats_port, metrics_port);
    let metrics_handle = tokio::spawn(async move {
        metrics::run(args, shutdown_rx).await.unwrap();
    });
    // allow the metrics tool to start
    sleep(Duration::from_secs(1)).await;

    let expected = ["peerobserver_runtime_start_timestamp "];
    assert!(check_metrics(metrics_port, &expected).expect("Could not fetch metrics"));

    shutdown_tx.send(true).unwrap();
    metrics_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_metrics_p2p_message_count() {
    println!("test that the P2P message count works");
    let (nats_port, metrics_port) = setup();
    let _nats_server = NatsServerForTesting::new(nats_port).await;
    let nats_publisher = NatsPublisherForTesting::new(nats_port).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let args = make_test_args(nats_port, metrics_port);
    let metrics_handle = tokio::spawn(async move {
        metrics::run(args, shutdown_rx).await.unwrap();
    });
    // allow the metrics tool to start
    sleep(Duration::from_secs(1)).await;

    let event1 = EventMsg::new(Event::Msg(net_msg::Message {
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
    .unwrap()
    .encode_to_vec();
    nats_publisher
        .publish(Subject::NetMsg.to_string(), event1)
        .await;

    let event2 = EventMsg::new(Event::Msg(net_msg::Message {
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
    .unwrap()
    .encode_to_vec();
    nats_publisher
        .publish(Subject::NetMsg.to_string(), event2)
        .await;

    sleep(Duration::from_millis(100)).await;

    let expected = r#"
peerobserver_p2p_message_bytes{connection_type="1",direction="inbound",message="ping"} 8
peerobserver_p2p_message_bytes{connection_type="1",direction="outbound",message="pong"} 8
peerobserver_p2p_message_bytes_by_subnet{direction="inbound",subnet="127.0.0.0"} 8
peerobserver_p2p_message_bytes_by_subnet{direction="outbound",subnet="127.0.0.0"} 8
peerobserver_p2p_message_count{connection_type="1",direction="inbound",message="ping"} 1
peerobserver_p2p_message_count{connection_type="1",direction="outbound",message="pong"} 1
peerobserver_p2p_message_count_by_subnet{direction="inbound",subnet="127.0.0.0"} 1
peerobserver_p2p_message_count_by_subnet{direction="outbound",subnet="127.0.0.0"} 1
peerobserver_p2p_ping_subnet{subnet="127.0.0.0"} 1
"#;

    let expected_lines: Vec<&str> = expected.split('\n').collect();
    assert!(check_metrics(metrics_port, &expected_lines).expect("Could not fetch metrics"));

    shutdown_tx.send(true).unwrap();
    metrics_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_metrics_conn_inbound() {
    println!("test that the inbound connection metrics work");
    let (nats_port, metrics_port) = setup();
    let _nats_server = NatsServerForTesting::new(nats_port).await;
    let nats_publisher = NatsPublisherForTesting::new(nats_port).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let args = make_test_args(nats_port, metrics_port);
    let metrics_handle = tokio::spawn(async move {
        metrics::run(args, shutdown_rx).await.unwrap();
    });
    // allow the metrics tool to start
    sleep(Duration::from_secs(1)).await;

    let event1 = EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
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
    .unwrap()
    .encode_to_vec();
    nats_publisher
        .publish(Subject::NetMsg.to_string(), event1)
        .await;

    sleep(Duration::from_millis(100)).await;

    let expected = r#"
peerobserver_conn_inbound 1
peerobserver_conn_inbound_current 123
peerobserver_conn_inbound_network{network="2"} 1
peerobserver_conn_inbound_subnet{subnet="127.0.0.0"} 1
"#;

    let expected_lines: Vec<&str> = expected.split('\n').collect();
    assert!(check_metrics(metrics_port, &expected_lines).expect("Could not fetch metrics"));

    shutdown_tx.send(true).unwrap();
    metrics_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_metrics_conn_outbound() {
    println!("test that the outbound connection metrics work");
    let (nats_port, metrics_port) = setup();
    let _nats_server = NatsServerForTesting::new(nats_port).await;
    let nats_publisher = NatsPublisherForTesting::new(nats_port).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let args = make_test_args(nats_port, metrics_port);
    let metrics_handle = tokio::spawn(async move {
        metrics::run(args, shutdown_rx).await.unwrap();
    });
    // allow the metrics tool to start
    sleep(Duration::from_secs(1)).await;

    let event1 = EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
        event: Some(net_conn::connection_event::Event::Outbound(
            shared::net_conn::OutboundConnection {
                conn: Connection {
                    addr: "1.1.1.1:48333".to_string(),
                    conn_type: 2,
                    network: 3,
                    peer_id: 11,
                },
                existing_connections: 321,
            },
        )),
    }))
    .unwrap()
    .encode_to_vec();
    nats_publisher
        .publish(Subject::NetMsg.to_string(), event1)
        .await;

    sleep(Duration::from_millis(100)).await;

    // also include "peerobserver_conn_inbound 0" here to make sure we never
    // have state from a previous test here
    let expected = r#"
peerobserver_conn_inbound 0
peerobserver_conn_outbound 1
peerobserver_conn_outbound_current 321
erobserver_conn_outbound_network{network="3"} 1
peerobserver_conn_outbound_subnet{subnet="1.1.1.0"} 1
"#;

    let expected_lines: Vec<&str> = expected.split('\n').collect();
    assert!(check_metrics(metrics_port, &expected_lines).expect("Could not fetch metrics"));

    shutdown_tx.send(true).unwrap();
    metrics_handle.await.unwrap();
}
