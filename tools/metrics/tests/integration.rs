#![cfg(feature = "nats_integration_tests")]

use metrics::error::RuntimeError;
use metrics::Args;

use shared::{
    log::{self, warn},
    nats_subjects::Subject,
    prost::Message,
    protobuf::{
        addrman::{self, InsertNew, InsertTried},
        event_msg::{event_msg::Event, EventMsg},
        log_extractor::{self, LogDebugCategory},
        mempool::{self, Added, Rejected, Removed, Replaced},
        net_conn::{
            self, ClosedConnection, Connection, EvictedInboundConnection, InboundConnection,
            MisbehavingConnection,
        },
        net_msg::{
            self, message::Msg, Addr, AddrV2, FeeFilter, Inv, Metadata, Ping, Pong, Reject, Version,
        },
        p2p_extractor,
        primitive::{self, inventory_item::Item, Address, InventoryItem},
        rpc::{self, MempoolInfo, PeerInfo, PeerInfos},
        validation::{self, BlockConnected},
    },
    rand::{self, Rng},
    simple_logger::SimpleLogger,
    testing::{nats_publisher::NatsPublisherForTesting, nats_server::NatsServerForTesting},
    tokio::{
        self,
        sync::{watch, Mutex},
        time::sleep,
    },
    util::current_timestamp,
};

use std::{
    collections::HashMap,
    io::ErrorKind,
    io::{Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc, Once, OnceLock,
    },
    time::Duration,
};

static INIT: Once = Once::new();
static NEXT_METRICS_PORT: OnceLock<AtomicU16> = OnceLock::new();

fn setup() -> u16 {
    INIT.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Trace)
            .init()
            .unwrap();

        let mut rng = rand::rng();

        // choose start ports from the ephemeral port range
        let metrics_start = rng.random_range(49152..65500);

        NEXT_METRICS_PORT
            .set(AtomicU16::new(metrics_start))
            .unwrap();
    });

    let metrics_port = NEXT_METRICS_PORT
        .get()
        .unwrap()
        .fetch_add(1, Ordering::SeqCst);
    return metrics_port;
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
    log::debug!("fetching metrics from {}", addr);
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

    println!("Only metrics (no help text):\n");
    for line in metrics_raw.split("\n") {
        if !line.starts_with("# ") {
            println!("{}", line);
        }
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

async fn publish_and_check(events: &[EventMsg], subject: Subject, expected: &str) {
    let initial_metrics_port = setup();
    let metrics_port: Arc<Mutex<u16>> = Arc::new(Mutex::new(initial_metrics_port));

    let nats_server = NatsServerForTesting::new().await;
    let nats_publisher = NatsPublisherForTesting::new(nats_server.port).await;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics_port_clone = metrics_port.clone();
    let metrics_handle = tokio::spawn(async move {
        loop {
            let port: u16;
            {
                port = *metrics_port_clone.lock().await;
            }

            let args = make_test_args(nats_server.port, port);
            match metrics::run(args, shutdown_rx.clone()).await {
                Ok(_) => break,
                Err(e) => match e {
                    RuntimeError::Io(e) => match e.kind() {
                        ErrorKind::AddrInUse => {
                            let new_port = NEXT_METRICS_PORT
                                .get()
                                .unwrap()
                                .fetch_add(1, Ordering::SeqCst);
                            warn!(
                                "Port {} seems to be already in use. Trying port {} next..",
                                port, new_port
                            );
                            let mut port = metrics_port_clone.lock().await;
                            *port = new_port;
                        }
                        _ => panic!("Couldn not start metrics tool: {}", e),
                    },
                    _ => panic!("Couldn not start metrics tool: {}", e),
                },
            }
        }
    });
    // allow the metrics tool to start
    sleep(Duration::from_secs(1)).await;

    for event in events {
        log::debug!("publishing: {:?}", event);
        nats_publisher
            .publish(subject.to_string(), event.encode_to_vec())
            .await;
    }

    sleep(Duration::from_millis(100)).await;

    let expected_lines: Vec<&str> = expected.split('\n').collect();
    let port = metrics_port.lock().await;
    assert!(check_metrics(*port, &expected_lines).expect("Could not fetch metrics"));

    shutdown_tx.send(true).unwrap();
    metrics_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_metrics_no_nats_connection() {
    println!("test that we fail if we can't connect to NATS (due to port 0)");
    let _ = setup();

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

    publish_and_check(
        &[],
        Subject::NetMsg, // not used
        "peerobserver_runtime_start_timestamp ",
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_message_count() {
    println!("test that the P2P message count works");

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
        r#"
        peerobserver_p2p_message_bytes{connection_type="1",direction="inbound",message="ping"} 8
        peerobserver_p2p_message_bytes{connection_type="1",direction="outbound",message="pong"} 8
        peerobserver_p2p_message_count{connection_type="1",direction="inbound",message="ping"} 1
        peerobserver_p2p_message_count{connection_type="1",direction="outbound",message="pong"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_traffic_linkinglion() {
    println!("test that the linkinglion traffic P2P metrics work");

    publish_and_check(
        &[
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 0,
                    addr: "162.218.65.123".to_string(), // an IP belonging to LinkingLion
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
                    addr: "91.198.115.23:8333".to_string(), // another IP belonging to LinkingLion
                    conn_type: 1,
                    command: "pong".to_string(),
                    inbound: true,
                    size: 8,
                },
                msg: Some(Msg::Pong(Pong { value: 1 })),
            }))
            .unwrap(),
        ],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_message_bytes{connection_type="1",direction="inbound",message="ping"} 8
        peerobserver_p2p_message_bytes{connection_type="1",direction="inbound",message="pong"} 8
        peerobserver_p2p_message_bytes_linkinglion{direction="inbound",message="ping"} 8
        peerobserver_p2p_message_bytes_linkinglion{direction="inbound",message="pong"} 8
        peerobserver_p2p_message_count{connection_type="1",direction="inbound",message="ping"} 1
        peerobserver_p2p_message_count{connection_type="1",direction="inbound",message="pong"} 1
        peerobserver_p2p_message_count_linkinglion{direction="inbound",message="ping"} 1
        peerobserver_p2p_message_count_linkinglion{direction="inbound",message="pong"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_addr() {
    println!("test that the P2P addr metrics work");

    let timestamp_now = current_timestamp() as u32;

    publish_and_check(
        &[EventMsg::new(Event::Msg(net_msg::Message {
            meta: Metadata {
                peer_id: 4,
                addr: "127.0.0.1:1234".to_string(),
                conn_type: 1,
                command: "addr".to_string(),
                inbound: true,
                size: 1234,
            },
            msg: Some(Msg::Addr(Addr {
                addresses: [
                    Address {
                        port: 1234,
                        services: 1234,
                        timestamp: timestamp_now + 200,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                    Address {
                        port: 2412,
                        services: 2311,
                        timestamp: timestamp_now,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                ]
                .to_vec(),
            })),
        }))
        .unwrap()],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="0"} 0
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="1"} 0
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="2"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="3"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="4"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="5"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="6"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="7"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="8"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="9"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="10"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="15"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="20"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="25"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="30"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="50"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="75"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="100"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="150"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="200"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="250"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="300"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="400"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="500"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="600"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="700"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="800"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="900"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="999"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="1000"} 1
        peerobserver_p2p_addr_addresses_bucket{direction="inbound",le="+Inf"} 1
        peerobserver_p2p_addr_addresses_sum{direction="inbound"} 2
        peerobserver_p2p_addr_addresses_count{direction="inbound"} 1
        peerobserver_p2p_addr_services{direction="inbound",services="1234"} 1
        peerobserver_p2p_addr_services{direction="inbound",services="2311"} 1
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="0"} 1
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="1"} 3
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="2"} 4
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="3"} 4
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="4"} 5
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="5"} 5
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="6"} 6
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="7"} 7
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="8"} 8
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="9"} 8
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="10"} 9
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="11"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="12"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="13"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="14"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="15"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="16"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="17"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="18"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="19"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="20"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="21"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="22"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="23"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="24"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="25"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="26"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="27"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="28"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="29"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="30"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="31"} 10
        peerobserver_p2p_addr_services_bits_bucket{direction="inbound",le="+Inf"} 10
        peerobserver_p2p_addr_services_bits_sum{direction="inbound"} 50
        peerobserver_p2p_addr_services_bits_count{direction="inbound"} 10
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="0"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="1"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="2"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="4"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="8"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="16"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="32"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="64"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="128"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="256"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="512"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="1024"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="2048"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="4096"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="8192"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="16384"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="32768"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="65536"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="131072"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="262144"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="524288"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="1048576"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="2097152"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="4194304"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="8388608"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="16777216"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="+Inf"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_sum{direction="inbound",timestamp_offset="future"} 200
        peerobserver_p2p_addr_timestamp_offset_seconds_count{direction="inbound",timestamp_offset="future"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="0"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="1"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="2"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="4"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="8"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="16"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="32"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="64"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="128"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="256"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="512"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="1024"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="2048"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="4096"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="8192"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="16384"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="32768"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="65536"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="131072"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="262144"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="524288"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="1048576"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="2097152"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="4194304"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="8388608"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="16777216"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="+Inf"} 1
        peerobserver_p2p_addr_timestamp_offset_seconds_sum{direction="inbound",timestamp_offset="past"} 0
        peerobserver_p2p_addr_timestamp_offset_seconds_count{direction="inbound",timestamp_offset="past"} 1
        peerobserver_p2p_message_bytes{connection_type="1",direction="inbound",message="addr"} 1234
        peerobserver_p2p_message_count{connection_type="1",direction="inbound",message="addr"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_addrv2() {
    println!("test that the P2P addrv2 metrics work");

    let timestamp_now = current_timestamp() as u32;

    publish_and_check(
        &[EventMsg::new(Event::Msg(net_msg::Message {
            meta: Metadata {
                peer_id: 8,
                addr: "127.0.0.1:1111".to_string(),
                conn_type: 2,
                command: "addrv2".to_string(),
                inbound: true,
                size: 5432,
            },
            msg: Some(Msg::Addrv2(AddrV2 {
                addresses: [
                    Address {
                        port: 1234,
                        services: 1234,
                        timestamp: timestamp_now + 512,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                    Address {
                        port: 2412,
                        services: 2311,
                        timestamp: timestamp_now,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                ]
                .to_vec(),
            })),
        }))
        .unwrap()],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="0"} 0
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="1"} 0
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="2"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="3"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="4"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="5"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="6"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="7"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="8"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="9"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="10"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="15"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="20"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="25"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="30"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="50"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="75"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="100"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="150"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="200"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="250"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="300"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="400"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="500"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="600"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="700"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="800"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="900"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="999"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="1000"} 1
        peerobserver_p2p_addrv2_addresses_bucket{direction="inbound",le="+Inf"} 1
        peerobserver_p2p_addrv2_addresses_sum{direction="inbound"} 2
        peerobserver_p2p_addrv2_addresses_count{direction="inbound"} 1
        peerobserver_p2p_addrv2_services{direction="inbound",services="1234"} 1
        peerobserver_p2p_addrv2_services{direction="inbound",services="2311"} 1
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="0"} 1
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="1"} 3
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="2"} 4
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="3"} 4
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="4"} 5
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="5"} 5
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="6"} 6
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="7"} 7
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="8"} 8
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="9"} 8
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="10"} 9
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="11"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="12"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="13"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="14"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="15"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="16"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="17"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="18"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="19"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="20"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="21"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="22"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="23"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="24"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="25"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="26"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="27"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="28"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="29"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="30"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="31"} 10
        peerobserver_p2p_addrv2_services_bits_bucket{direction="inbound",le="+Inf"} 10
        peerobserver_p2p_addrv2_services_bits_sum{direction="inbound"} 50
        peerobserver_p2p_addrv2_services_bits_count{direction="inbound"} 10
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="0"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="1"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="2"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="4"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="8"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="16"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="32"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="64"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="128"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="256"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="512"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="1024"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="2048"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="4096"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="8192"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="16384"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="32768"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="65536"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="131072"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="262144"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="524288"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="1048576"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="2097152"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="4194304"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="8388608"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="16777216"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="future",le="+Inf"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_sum{direction="inbound",timestamp_offset="future"} 512
        peerobserver_p2p_addrv2_timestamp_offset_seconds_count{direction="inbound",timestamp_offset="future"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="0"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="1"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="2"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="4"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="8"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="16"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="32"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="64"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="128"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="256"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="512"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="1024"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="2048"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="4096"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="8192"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="16384"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="32768"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="65536"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="131072"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="262144"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="524288"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="1048576"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="2097152"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="4194304"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="8388608"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="16777216"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_bucket{direction="inbound",timestamp_offset="past",le="+Inf"} 1
        peerobserver_p2p_addrv2_timestamp_offset_seconds_sum{direction="inbound",timestamp_offset="past"} 0
        peerobserver_p2p_addrv2_timestamp_offset_seconds_count{direction="inbound",timestamp_offset="past"} 1
        peerobserver_p2p_message_bytes{connection_type="2",direction="inbound",message="addrv2"} 5432
        peerobserver_p2p_message_count{connection_type="2",direction="inbound",message="addrv2"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_version() {
    println!("test that the P2P version metrics work");

    let timestamp_now = current_timestamp() as u32;

    publish_and_check(
        &[
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 6,
                    addr: "127.0.0.1:9999".to_string(),
                    conn_type: 2,
                    command: "version".to_string(),
                    inbound: true,
                    size: 2,
                },
                msg: Some(Msg::Version(Version {
                    nonce: 2,
                    receiver: Address {
                        port: 1234,
                        services: 1234,
                        timestamp: timestamp_now + 512,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                    sender: Address {
                        port: 1234,
                        services: 1234,
                        timestamp: timestamp_now + 512,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                    services: 2341,
                    start_height: 1,
                    relay: false,
                    timestamp: timestamp_now as i64 - 10,
                    user_agent: "user_agent".to_string(),
                    version: 70016,
                })),
            }))
            .unwrap(),
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 2,
                    addr: "162.218.65.123:1234".to_string(),
                    conn_type: 1,
                    command: "version".to_string(),
                    inbound: true,
                    size: 1,
                },
                msg: Some(Msg::Version(Version {
                    nonce: 2,
                    receiver: Address {
                        port: 1234,
                        services: 1234,
                        timestamp: timestamp_now + 512,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                    sender: Address {
                        port: 1234,
                        services: 1234,
                        timestamp: timestamp_now + 512,
                        address: Some(primitive::address::Address::Ipv4(String::from("127.0.0.1"))),
                    },
                    services: 2341,
                    start_height: 1,
                    relay: false,
                    timestamp: timestamp_now as i64 - 10,
                    user_agent: "user_agent".to_string(),
                    version: 70016,
                })),
            }))
            .unwrap(),
        ],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_message_bytes{connection_type="1",direction="inbound",message="version"} 1
        peerobserver_p2p_message_bytes{connection_type="2",direction="inbound",message="version"} 2
        peerobserver_p2p_message_bytes_linkinglion{direction="inbound",message="version"} 1
        peerobserver_p2p_message_count{connection_type="1",direction="inbound",message="version"} 1
        peerobserver_p2p_message_count{connection_type="2",direction="inbound",message="version"} 1
        peerobserver_p2p_message_count_linkinglion{direction="inbound",message="version"} 1
        peerobserver_p2p_version_useragent{useragent="LinkingLion"} 1
        peerobserver_p2p_version_useragent{useragent="user_agent"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_feefilter() {
    println!("test that the P2P feefilter metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Msg(net_msg::Message {
            meta: Metadata {
                peer_id: 6,
                addr: "127.0.0.1:2134".to_string(),
                conn_type: 5,
                command: "feefilter".to_string(),
                inbound: true,
                size: 6,
            },
            msg: Some(Msg::Feefilter(FeeFilter {
                fee: 12345
            })),
        }))
        .unwrap()],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_feefilter_feerate{direction="inbound",feerate="12345"} 1
        peerobserver_p2p_message_bytes{connection_type="5",direction="inbound",message="feefilter"} 6
        peerobserver_p2p_message_count{connection_type="5",direction="inbound",message="feefilter"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_rejected() {
    println!("test that the P2P rejected metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Msg(net_msg::Message {
            meta: Metadata {
                peer_id: 6,
                addr: "127.0.0.1:2134".to_string(),
                conn_type: 5,
                command: "rejected".to_string(),
                inbound: true,
                size: 6,
            },
            msg: Some(Msg::Reject(Reject {
                reason: 1,
                reason_details: "details".to_string(),
                rejected_command: "tx".to_string(),
                hash: vec![],
            })),
        }))
        .unwrap(),
        EventMsg::new(Event::Msg(net_msg::Message {
            meta: Metadata {
                peer_id: 6,
                addr: "127.0.0.1:2134".to_string(),
                conn_type: 5,
                command: "rejected".to_string(),
                inbound: true,
                size: 6,
            },
            msg: Some(Msg::Reject(Reject {
                reason: 10000,
                reason_details: "details".to_string(),
                rejected_command: "tx".to_string(),
                hash: vec![],
            })),
        }))
        .unwrap()],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_message_bytes{connection_type="5",direction="inbound",message="rejected"} 12
        peerobserver_p2p_message_count{connection_type="5",direction="inbound",message="rejected"} 2
        peerobserver_p2p_reject_message{rejectcommand="tx",rejectreason="Invalid"} 1
        peerobserver_p2p_reject_message{rejectcommand="tx",rejectreason="unknown"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_inv() {
    println!("test that the P2P inv metrics work");

    publish_and_check(
        &[
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 1,
                    addr: "127.0.0.1:2134".to_string(),
                    conn_type: 3,
                    command: "inv".to_string(),
                    inbound: true,
                    size: 80,
                },
                msg: Some(Msg::Inv(Inv {
                    // homogenus
                    items: [
                        InventoryItem {
                            item: Some(Item::Transaction(vec![])),
                        },
                        InventoryItem {
                            item: Some(Item::Transaction(vec![])),
                        },
                    ]
                    .to_vec(),
                })),
            }))
            .unwrap(),
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 1,
                    addr: "127.0.0.1:2134".to_string(),
                    conn_type: 3,
                    command: "inv".to_string(),
                    inbound: true,
                    size: 80,
                },
                msg: Some(Msg::Inv(Inv {
                    // heterogenous
                    items: [
                        InventoryItem {
                            item: Some(Item::Transaction(vec![])),
                        },
                        InventoryItem {
                            item: Some(Item::Block(vec![])),
                        },
                    ]
                    .to_vec(),
                })),
            }))
            .unwrap(),
        ],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_inv_entries{direction="inbound",inv_type="Block"} 1
        peerobserver_p2p_inv_entries{direction="inbound",inv_type="Tx"} 3
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="0"} 0
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="1"} 0
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="2"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="3"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="4"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="5"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="6"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="7"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="8"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="9"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="10"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="15"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="20"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="25"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="30"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="50"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="75"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="100"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="150"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="200"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="250"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="300"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="400"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="500"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="600"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="700"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="800"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="900"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="999"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="1000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="2000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="3000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="4000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="5000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="6000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="7000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="8000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="9000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="10000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="20000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="25000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="30000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="35000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="40000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="45000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="50000"} 2
        peerobserver_p2p_inv_entries_histogram_bucket{direction="inbound",le="+Inf"} 2
        peerobserver_p2p_inv_entries_histogram_sum{direction="inbound"} 4
        peerobserver_p2p_inv_entries_histogram_count{direction="inbound"} 2
        peerobserver_p2p_invs_heterogeneous{direction="inbound"} 1
        peerobserver_p2p_invs_homogeneous{direction="inbound"} 1
        peerobserver_p2p_message_bytes{connection_type="3",direction="inbound",message="inv"} 160
        peerobserver_p2p_message_count{connection_type="3",direction="inbound",message="inv"} 2
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_inv_large_outbound() {
    println!("test that large outbound INV message (> 35) metrics work");

    let large_inv_items_tx: Vec<InventoryItem> = (0..40)
        .map(|_| InventoryItem {
            item: Some(Item::Transaction(vec![])),
        })
        .collect();

    let large_inv_items_wtx: Vec<InventoryItem> = (0..36)
        .map(|_| InventoryItem {
            item: Some(Item::Wtx(vec![])),
        })
        .collect();

    publish_and_check(
        &[
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 1,
                    addr: "127.0.0.1:2134".to_string(),
                    conn_type: 3,
                    command: "inv".to_string(),
                    inbound: false,
                    size: 1000,
                },
                msg: Some(Msg::Inv(Inv {
                    items: large_inv_items_tx,
                })),
            }))
            .unwrap(),
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 2,
                    addr: "127.0.0.1:2134".to_string(),
                    conn_type: 3,
                    command: "inv".to_string(),
                    inbound: false,
                    size: 1000,
                },
                msg: Some(Msg::Inv(Inv {
                    items: large_inv_items_wtx,
                })),
            }))
            .unwrap(),
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 3,
                    addr: "127.0.0.1:2134".to_string(),
                    conn_type: 3,
                    command: "inv".to_string(),
                    inbound: false,
                    size: 80,
                },
                msg: Some(Msg::Inv(Inv {
                    items: [
                        InventoryItem {
                            item: Some(Item::Transaction(vec![])),
                        },
                        InventoryItem {
                            item: Some(Item::Transaction(vec![])),
                        },
                    ]
                    .to_vec(),
                })),
            }))
            .unwrap(),
        ],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_invs_outbound_large 2
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_oldping() {
    println!("test that the P2P oldping metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Msg(net_msg::Message {
            meta: Metadata {
                peer_id: 6,
                addr: "127.0.0.1:2134".to_string(),
                conn_type: 2,
                command: "ping".to_string(),
                inbound: true,
                size: 0,
            },
            msg: Some(Msg::Oldping(false)),
        }))
        .unwrap()],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_message_bytes{connection_type="2",direction="inbound",message="ping"} 0
        peerobserver_p2p_message_count{connection_type="2",direction="inbound",message="ping"} 1
        peerobserver_p2p_oldping_subnet{subnet="127.0.0.0"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_ping_value() {
    println!("test that the P2P ping value metrics work");

    let values = vec![
        0,                   // => 0
        1,                   // => u8
        u8::MAX as u64 - 1,  // => u8
        u8::MAX as u64,      // => u8
        u8::MAX as u64 + 1,  // => u16
        u16::MAX as u64 - 1, // => u16
        u16::MAX as u64,     // => u16
        u16::MAX as u64 + 1, // => u32
        u32::MAX as u64 - 1, // => u32
        u32::MAX as u64,     // => u32
        u32::MAX as u64 + 1, // => u64
        u64::MAX as u64 - 1, // => u64
        u64::MAX as u64,     // => u64
    ];

    let events: Vec<EventMsg> = values
        .iter()
        .map(|v| {
            EventMsg::new(Event::Msg(net_msg::Message {
                meta: Metadata {
                    peer_id: 6,
                    addr: "127.0.0.1:2134".to_string(),
                    conn_type: 2,
                    command: "ping".to_string(),
                    inbound: true,
                    size: 8,
                },
                msg: Some(Msg::Ping(Ping { value: *v })),
            }))
            .unwrap()
        })
        .collect();

    publish_and_check(
        &events,
        Subject::NetMsg,
        r#"
        peerobserver_p2p_ping_inbound_value{value="0"} 1
        peerobserver_p2p_ping_inbound_value{value="u8"} 3
        peerobserver_p2p_ping_inbound_value{value="u16"} 3
        peerobserver_p2p_ping_inbound_value{value="u32"} 3
        peerobserver_p2p_ping_inbound_value{value="u64"} 3
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2p_empty_addrv2() {
    println!("test that the P2P emptyaddrv2 metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Msg(net_msg::Message {
            meta: Metadata {
                peer_id: 6,
                addr: "127.0.0.1:2134".to_string(),
                conn_type: 2,
                command: "addrv2".to_string(),
                inbound: true,
                size: 0,
            },
            msg: Some(Msg::Emptyaddrv2(false)),
        }))
        .unwrap()],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_addrv2_empty{addr="127.0.0.1",direction="inbound"} 1
        peerobserver_p2p_message_bytes{connection_type="2",direction="inbound",message="addrv2"} 0
        peerobserver_p2p_message_count{connection_type="2",direction="inbound",message="addrv2"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_conn_inbound() {
    println!("test that the inbound connection metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Inbound(
                InboundConnection {
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
        .unwrap()],
        Subject::NetConn,
        r#"
        peerobserver_conn_inbound 1
        peerobserver_conn_inbound_current 123
        peerobserver_conn_inbound_network{network="2"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_conn_outbound() {
    println!("test that the outbound connection metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Outbound(
                shared::protobuf::net_conn::OutboundConnection {
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
        .unwrap()],
        Subject::NetConn,
        r#"
        peerobserver_conn_inbound 0
        peerobserver_conn_outbound 1
        peerobserver_conn_outbound_current 321
        erobserver_conn_outbound_network{network="3"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_conn_closed() {
    println!("test that the closed connection metrics work");

    let timestamp_now = current_timestamp();

    publish_and_check(
        &[EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Closed(
                ClosedConnection {
                    conn: Connection {
                        addr: "2.2.2.2:48333".to_string(),
                        conn_type: 4,
                        network: 4,
                        peer_id: 1,
                    },
                    time_established: timestamp_now - 1000,
                },
            )),
        }))
        .unwrap()],
        Subject::NetConn,
        r#"
        peerobserver_conn_closed 1
        peerobserver_conn_closed_age 1000
        peerobserver_conn_closed_network{network="4"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_conn_inbound_evicted() {
    println!("test that the inbound_evicted connection metrics work");

    let timestamp_now = current_timestamp();

    publish_and_check(
        &[EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::InboundEvicted(
                EvictedInboundConnection {
                    conn: Connection {
                        addr: "2.2.2.2:48333".to_string(),
                        conn_type: 2,
                        network: 2,
                        peer_id: 1,
                    },
                    time_established: timestamp_now - 1000,
                },
            )),
        }))
        .unwrap()],
        Subject::NetConn,
        r#"
        peerobserver_conn_evicted_inbound 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_conn_misbehaving() {
    println!("test that the misbehaving connection metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
            event: Some(net_conn::connection_event::Event::Misbehaving(
                MisbehavingConnection {
                    id: 2,
                    message: "reason".to_string(),
                },
            )),
        }))
        .unwrap()],
        Subject::NetConn,
        r#"
        peerobserver_conn_misbehaving{id="2",misbehavingmessage="reason"} 1
        peerobserver_conn_misbehaving_reason{misbehavingmessage="reason"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_conn_special_ip() {
    println!("test that the metrics for special IPs work");

    publish_and_check(
        &[
            EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
                event: Some(net_conn::connection_event::Event::Inbound(
                    InboundConnection {
                        conn: Connection {
                            addr: "162.218.65.123".to_string(), // an IP belonging to LinkingLion & banned on Gmax banlist
                            conn_type: 1,
                            network: 2,
                            peer_id: 7,
                        },
                        existing_connections: 1,
                    },
                )),
            }))
            .unwrap(),
            EventMsg::new(Event::Conn(net_conn::ConnectionEvent {
                event: Some(net_conn::connection_event::Event::Inbound(
                    InboundConnection {
                        conn: Connection {
                            // a random IP belonging to a tor exit node.
                            // This might not be a tor exit node IP in the future and the IP would need to updated.
                            addr: "179.43.182.232:1234".to_string(),
                            conn_type: 1,
                            network: 2,
                            peer_id: 7,
                        },
                        existing_connections: 2,
                    },
                )),
            }))
            .unwrap(),
        ],
        Subject::NetConn,
        r#"
        peerobserver_conn_inbound 2
        peerobserver_conn_inbound_banlist_gmax 1
        peerobserver_conn_inbound_banlist_monero 1
        peerobserver_conn_inbound_list_linkinglion 1
        peerobserver_conn_inbound_current 2
        peerobserver_conn_inbound_network{network="2"} 2
        peerobserver_conn_inbound_tor_exit 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_validation() {
    println!("test that validation metrics work");

    publish_and_check(
        &[
            EventMsg::new(Event::Validation(validation::ValidationEvent {
                event: Some(validation::validation_event::Event::BlockConnected(
                    BlockConnected {
                        hash: vec![0],
                        height: 1337,
                        transactions: 13,
                        inputs: 3,
                        sigops: 7,
                        connection_time: 5000,
                    },
                )),
            }))
            .unwrap(),
        ],
        Subject::Validation,
        r#"
        peerobserver_validation_block_connected_connection_time 5
        peerobserver_validation_block_connected_latest_connection_time 5
        peerobserver_validation_block_connected_latest_height 1337
        peerobserver_validation_block_connected_latest_inputs 3
        peerobserver_validation_block_connected_latest_sigops 7
        peerobserver_validation_block_connected_latest_transactions 13
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_mempool_added() {
    println!("test that the mempool added metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Mempool(mempool::MempoolEvent {
            event: Some(mempool::mempool_event::Event::Added(Added {
                fee: 0,       // not covered by test
                txid: vec![], // not covered by test
                vsize: 453,
            })),
        }))
        .unwrap()],
        Subject::Mempool,
        r#"
        peerobserver_mempool_added 1
        peerobserver_mempool_added_vbytes 453
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_mempool_added_mass() {
    println!("test that the mempool added metrics work when we add a lot of events");

    let events: Vec<EventMsg> = (0..54321)
        .map(|_| {
            EventMsg::new(Event::Mempool(mempool::MempoolEvent {
                event: Some(mempool::mempool_event::Event::Added(Added {
                    fee: 0,       // not covered by test
                    txid: vec![], // not covered by test
                    vsize: 100,
                })),
            }))
            .unwrap()
        })
        .collect();

    publish_and_check(
        &events,
        Subject::Mempool,
        r#"
        peerobserver_mempool_added 54321
        peerobserver_mempool_added_vbytes 5432100
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_mempool_replaced() {
    println!("test that the mempool replaced metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Mempool(mempool::MempoolEvent {
            event: Some(mempool::mempool_event::Event::Replaced(Replaced {
                replaced_fee: 0, // not covered by test
                replaced_vsize: 17,
                replaced_entry_time: 0,        // not covered by test
                replaced_txid: vec![],         // not covered by test
                replacement_id: vec![],        // not covered by test
                replacement_vsize: 0,          // not covered by test
                replacement_fee: 0,            // not covered by test
                replaced_by_transaction: true, // not covered by test
            })),
        }))
        .unwrap()],
        Subject::Mempool,
        r#"
        peerobserver_mempool_replaced 1
        peerobserver_mempool_replaced_vbytes 17
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_mempool_rejected() {
    println!("test that the mempool rejected metrics work");

    publish_and_check(
        &[
            EventMsg::new(Event::Mempool(mempool::MempoolEvent {
                event: Some(mempool::mempool_event::Event::Rejected(Rejected {
                    reason: "ABC".to_string(),
                    txid: vec![], // not covered by test
                })),
            }))
            .unwrap(),
            EventMsg::new(Event::Mempool(mempool::MempoolEvent {
                event: Some(mempool::mempool_event::Event::Rejected(Rejected {
                    reason: "DEF".to_string(),
                    txid: vec![], // not covered by test
                })),
            }))
            .unwrap(),
        ],
        Subject::Mempool,
        r#"
        peerobserver_mempool_rejected{reason="ABC"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_mempool_removed() {
    println!("test that the mempool removed metrics work");

    publish_and_check(
        &[
            EventMsg::new(Event::Mempool(mempool::MempoolEvent {
                event: Some(mempool::mempool_event::Event::Removed(Removed {
                    entry_time: 0, // not covered by test
                    fee: 0,        // not covered by test
                    vsize: 0,      // not covered by test
                    txid: vec![],  // not covered by test
                    reason: "expired".to_string(),
                })),
            }))
            .unwrap(),
            EventMsg::new(Event::Mempool(mempool::MempoolEvent {
                event: Some(mempool::mempool_event::Event::Removed(Removed {
                    entry_time: 0, // not covered by test
                    fee: 0,        // not covered by test
                    vsize: 0,      // not covered by test
                    txid: vec![],  // not covered by test
                    reason: "evicted".to_string(),
                })),
            }))
            .unwrap(),
        ],
        Subject::Mempool,
        r#"
        peerobserver_mempool_removed{reason="expired"} 1
        peerobserver_mempool_removed{reason="evicted"} 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_rpc_peerinfo() {
    println!("test that the RPC peer-info metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Rpc(rpc::RpcEvent {
            event: Some(rpc::rpc_event::Event::PeerInfos(PeerInfos {
                infos: vec![
                    PeerInfo {
                        addr_processed: 1234,
                        addr_rate_limited: 1234,
                        addr_relay_enabled: false,
                        // a random IP belonging to a tor exit node.
                        // This might not be a tor exit node IP in the future and the IP would need to updated.
                        address: "179.43.182.232:1234".to_string(),
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: true,
                        bip152_hb_to: false,
                        bytes_received: 1,
                        bytes_received_per_message: HashMap::new(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 0,
                        connection_time: 1,
                        connection_type: "type0".to_string(),
                        id: 1,
                        inbound: true,
                        inflight: vec![1337, 45324],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 1234,
                        minfeefilter: 1234.0,
                        minimum_ping: 1234.0,
                        network: "ipv4".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 1234.0,
                        ping_wait: 1234.0,
                        relay_transactions: true,
                        services: "service".to_string(),
                        starting_height: 1337,
                        subversion: "subversion".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: 1234,
                        transport_protocol_type: "v1".to_string(),
                        version: 2841,
                    },
                    PeerInfo {
                        addr_processed: 342,
                        addr_rate_limited: 0,
                        addr_relay_enabled: true,
                        address: "162.218.65.123:8332".to_string(), // LinkingLion IP
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: false,
                        bip152_hb_to: true,
                        bytes_received: 2344,
                        bytes_received_per_message: HashMap::new(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 3483,
                        connection_time: 8432,
                        connection_type: "type1".to_string(),
                        id: 2,
                        inbound: false,
                        inflight: vec![],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 0,
                        minfeefilter: 2.0,
                        minimum_ping: 13.0,
                        network: "ipv6".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 23.0,
                        ping_wait: 53.0,
                        relay_transactions: false,
                        services: "service".to_string(),
                        starting_height: 231,
                        subversion: "subversion2".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: -1239,
                        transport_protocol_type: "v2".to_string(),
                        version: 2342,
                    },
                    PeerInfo {
                        addr_processed: 342,
                        addr_rate_limited: 434,
                        addr_relay_enabled: true,
                        address: "162.218.65.123:8332".to_string(), // LinkingLion IP
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: false,
                        bip152_hb_to: true,
                        bytes_received: 2344,
                        bytes_received_per_message: HashMap::new(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 3483,
                        connection_time: 8432,
                        connection_type: "type1".to_string(),
                        id: 2,
                        inbound: false,
                        inflight: vec![],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 1234,
                        minfeefilter: 2.0,
                        minimum_ping: 13.0,
                        network: "ipv6".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 23.0,
                        ping_wait: 53.0,
                        relay_transactions: false,
                        services: "service".to_string(),
                        starting_height: 231,
                        subversion: "subversion2".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: -1239,
                        transport_protocol_type: "v2".to_string(),
                        version: 2342,
                    },
                ],
            })),
        }))
        .unwrap()],
        Subject::Rpc,
        r#"
        peerobserver_rpc_peer_info_addr_processed_total 1918
        peerobserver_rpc_peer_info_addr_ratelimited_peers 2
        peerobserver_rpc_peer_info_addr_ratelimited_total 1668
        peerobserver_rpc_peer_info_addr_relay_enabled_peers 2
        peerobserver_rpc_peer_info_asn_peers{ASN="1234"} 2
        peerobserver_rpc_peer_info_bip152_highbandwidth_from 1
        peerobserver_rpc_peer_info_bip152_highbandwidth_to 2
        peerobserver_rpc_peer_info_connection_type_peers{connection_type="type0"} 1
        peerobserver_rpc_peer_info_connection_type_peers{connection_type="type1"} 2
        peerobserver_rpc_peer_info_inflight_block_peers 1
        peerobserver_rpc_peer_info_inflight_distinct_blocks_heights 2
        peerobserver_rpc_peer_info_list_peers_gmax_ban 2
        peerobserver_rpc_peer_info_list_peers_linkinglion 2
        peerobserver_rpc_peer_info_list_peers_monero_ban 2
        peerobserver_rpc_peer_info_list_peers_tor_exit 1
        peerobserver_rpc_peer_info_minping_mean 420000
        peerobserver_rpc_peer_info_minping_median 13000
        peerobserver_rpc_peer_info_network_peers{network="ipv4"} 1
        peerobserver_rpc_peer_info_network_peers{network="ipv6"} 2
        peerobserver_rpc_peer_info_num_peers 3
        peerobserver_rpc_peer_info_ping_mean 426666.6666666667
        peerobserver_rpc_peer_info_ping_median 23000
        peerobserver_rpc_peer_info_ping_wait_larger_5_seconds_block_peers 3
        peerobserver_rpc_peer_info_protocol_version_peers{protocol_version="2342"} 2
        peerobserver_rpc_peer_info_protocol_version_peers{protocol_version="2841"} 1
        peerobserver_rpc_peer_info_timeoffset_minus10s 2
        peerobserver_rpc_peer_info_timeoffset_plus10s 1
        peerobserver_rpc_peer_info_transport_protocol_type_peers{transport_protocol_type="v1"} 1
        peerobserver_rpc_peer_info_transport_protocol_type_peers{transport_protocol_type="v2"} 2
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_addrman() {
    println!("test that the addrman metrics work");

    publish_and_check(
        &[
            EventMsg::new(Event::Addrman(addrman::AddrmanEvent {
                event: Some(addrman::addrman_event::Event::New(InsertNew {
                    addr: "127.0.0.1:2340".to_string(),
                    addr_as: 2,
                    bucket: 2,
                    bucket_pos: 2,
                    inserted: false,
                    source: "127.0.0.1:2340".to_string(),
                    source_as: 0,
                })),
            }))
            .unwrap(),
            EventMsg::new(Event::Addrman(addrman::AddrmanEvent {
                event: Some(addrman::addrman_event::Event::Tried(InsertTried {
                    addr: "127.0.0.1:2340".to_string(),
                    addr_as: 2,
                    bucket: 2,
                    bucket_pos: 2,
                    source: "127.0.0.1:2340".to_string(),
                    source_as: 0,
                })),
            }))
            .unwrap(),
        ],
        Subject::Addrman,
        r#"
        peerobserver_addrman_new_insert{inserted="false"} 1
        peerobserver_addrman_tried_insert 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_rpc_peerinfo_sub1satvbyte() {
    println!("test that the sub-1 sat/vbyte peers metric works");

    let mut bytes_received_per_message = HashMap::new();
    bytes_received_per_message.insert("tx".to_string(), 1234);

    publish_and_check(
        &[EventMsg::new(Event::Rpc(rpc::RpcEvent {
            event: Some(rpc::rpc_event::Event::PeerInfos(PeerInfos {
                infos: vec![
                    // This peer is a sub-1 sat/vbyte peer as the minfeefilter is 0.1 sat/vbyte
                    // and it has received txns (bytes_received_per_message).
                    PeerInfo {
                        addr_processed: 1234,
                        addr_rate_limited: 1234,
                        addr_relay_enabled: false,
                        // a random IP belonging to a tor exit node.
                        // This might not be a tor exit node IP in the future and the IP would need to updated.
                        address: "179.43.182.232:1234".to_string(),
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: true,
                        bip152_hb_to: false,
                        bytes_received: 1,
                        bytes_received_per_message: bytes_received_per_message.clone(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 0,
                        connection_time: 1,
                        connection_type: "type0".to_string(),
                        id: 1,
                        inbound: true,
                        inflight: vec![1337, 45324],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 1234,
                        minfeefilter: 0.000001, // 0.1 sat/vbyte
                        minimum_ping: 1234.0,
                        network: "ipv4".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 1234.0,
                        ping_wait: 1234.0,
                        relay_transactions: true,
                        services: "service".to_string(),
                        starting_height: 1337,
                        subversion: "subversion".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: 1234,
                        transport_protocol_type: "v1".to_string(),
                        version: 2841,
                    },
                    // This peer is not a sub-1 sat/vbyte peer as the minfeefilter is 1 sat/vbyte.
                    PeerInfo {
                        addr_processed: 342,
                        addr_rate_limited: 0,
                        addr_relay_enabled: true,
                        address: "162.218.65.123:8332".to_string(), // LinkingLion IP
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: false,
                        bip152_hb_to: true,
                        bytes_received: 2344,
                        bytes_received_per_message: bytes_received_per_message.clone(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 3483,
                        connection_time: 8432,
                        connection_type: "type1".to_string(),
                        id: 2,
                        inbound: false,
                        inflight: vec![],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 0,
                        minfeefilter: 0.00001, // 1 sat/vbyte,
                        minimum_ping: 13.0,
                        network: "ipv6".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 23.0,
                        ping_wait: 53.0,
                        relay_transactions: false,
                        services: "service".to_string(),
                        starting_height: 231,
                        subversion: "subversion2".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: -1239,
                        transport_protocol_type: "v2".to_string(),
                        version: 2342,
                    },
                    // This peer is not counted as a sub-1 sat/vbyte peer even if the the minfeefilter is 0.5 sat/vbyte.
                    // It didn't receive or send any tx yet.
                    PeerInfo {
                        addr_processed: 342,
                        addr_rate_limited: 434,
                        addr_relay_enabled: true,
                        address: "162.218.65.123:8332".to_string(), // LinkingLion IP
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: false,
                        bip152_hb_to: true,
                        bytes_received: 2344,
                        bytes_received_per_message: HashMap::new(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 3483,
                        connection_time: 8432,
                        connection_type: "type1".to_string(),
                        id: 2,
                        inbound: false,
                        inflight: vec![],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 1234,
                        minfeefilter: 0.000005, // 0.5 sat/vbyte,
                        minimum_ping: 13.0,
                        network: "ipv6".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 23.0,
                        ping_wait: 53.0,
                        relay_transactions: false,
                        services: "service".to_string(),
                        starting_height: 231,
                        subversion: "subversion2".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: -1239,
                        transport_protocol_type: "v2".to_string(),
                        version: 2342,
                    },
                ],
            })),
        }))
        .unwrap()],
        Subject::Rpc,
        r#"
        peerobserver_rpc_peer_info_sub1satvb_relay 1
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_rpc_peerinfo_ipv4_inbound_diversity() {
    println!("test that the ipv4 inbound diversity metric works");

    publish_and_check(
        &[EventMsg::new(Event::Rpc(rpc::RpcEvent {
            event: Some(rpc::rpc_event::Event::PeerInfos(PeerInfos {
                infos: vec![
                    // The first two peers are from the same /16 (123.123.*) and
                    // the third peer is from a distict (234.234.*) subnet. This results
                    // in a 2/3 diversity metric.
                    PeerInfo {
                        addr_processed: 1234,
                        addr_rate_limited: 1234,
                        addr_relay_enabled: false,
                        address: "123.123.123.123:1234".to_string(),
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: true,
                        bip152_hb_to: false,
                        bytes_received: 1,
                        bytes_received_per_message: HashMap::new(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 0,
                        connection_time: 1,
                        connection_type: "type0".to_string(),
                        id: 1,
                        inbound: true,
                        inflight: vec![1337, 45324],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 1234,
                        minfeefilter: 1.0,
                        minimum_ping: 1234.0,
                        network: "ipv4".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 1234.0,
                        ping_wait: 1234.0,
                        relay_transactions: true,
                        services: "service".to_string(),
                        starting_height: 1337,
                        subversion: "subversion".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: 1234,
                        transport_protocol_type: "v1".to_string(),
                        version: 2841,
                    },
                    PeerInfo {
                        addr_processed: 342,
                        addr_rate_limited: 0,
                        addr_relay_enabled: true,
                        address: "123.123.123.234:8332".to_string(),
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: false,
                        bip152_hb_to: true,
                        bytes_received: 2344,
                        bytes_received_per_message: HashMap::new(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 3483,
                        connection_time: 8432,
                        connection_type: "type1".to_string(),
                        id: 2,
                        inbound: true,
                        inflight: vec![],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 0,
                        minfeefilter: 1.0,
                        minimum_ping: 13.0,
                        network: "ipv4".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 23.0,
                        ping_wait: 53.0,
                        relay_transactions: false,
                        services: "service".to_string(),
                        starting_height: 231,
                        subversion: "subversion2".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: -1239,
                        transport_protocol_type: "v2".to_string(),
                        version: 2342,
                    },
                    PeerInfo {
                        addr_processed: 342,
                        addr_rate_limited: 434,
                        addr_relay_enabled: true,
                        address: "234.234.234.234:8332".to_string(),
                        address_bind: "1.2.3.4:8332".to_string(),
                        address_local: "1.2.3.4:8332".to_string(),
                        bip152_hb_from: false,
                        bip152_hb_to: true,
                        bytes_received: 2344,
                        bytes_received_per_message: HashMap::new(),
                        bytes_sent_per_message: HashMap::new(),
                        bytes_sent: 3483,
                        connection_time: 8432,
                        connection_type: "type1".to_string(),
                        id: 2,
                        inbound: true,
                        inflight: vec![],
                        last_block: 1337,
                        last_received: 1234,
                        last_send: 1234,
                        last_transaction: 1234,
                        mapped_as: 1234,
                        minfeefilter: 1.0,
                        minimum_ping: 13.0,
                        network: "ipv4".to_string(),
                        permissions: vec!["permission".to_string()],
                        ping_time: 23.0,
                        ping_wait: 53.0,
                        relay_transactions: false,
                        services: "service".to_string(),
                        starting_height: 231,
                        subversion: "subversion2".to_string(),
                        synced_blocks: 4,
                        synced_headers: 5,
                        time_offset: -1239,
                        transport_protocol_type: "v2".to_string(),
                        version: 2342,
                    },
                ],
            })),
        }))
        .unwrap()],
        Subject::Rpc,
        r#"
        peerobserver_rpc_peer_info_connection_divserity_inbound_ipv4 0.6666666666666666
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_rpc_mempolinfo() {
    println!("test that the mempoolinfo metrics work");

    publish_and_check(
        &[EventMsg::new(Event::Rpc(rpc::RpcEvent {
            event: Some(rpc::rpc_event::Event::MempoolInfo(MempoolInfo {
                loaded: true,
                size: 1000,
                bytes: 2000,
                usage: 3000,
                total_fee: 4000.1,
                max_mempool: 5000,
                incrementalrelayfee: 6.0001,
                mempoolminfee: 7.2,
                minrelaytxfee: 8.3,
                unbroadcastcount: 0, // not covered
                fullrbf: false,      // not covered
            })),
        }))
        .unwrap()],
        Subject::Rpc,
        r#"
        peerobserver_rpc_mempoolinfo_incremental_relay_feerate 6.0001
        peerobserver_rpc_mempoolinfo_memory_max 5000
        peerobserver_rpc_mempoolinfo_memory_usage 3000
        peerobserver_rpc_mempoolinfo_mempool_loaded 1
        peerobserver_rpc_mempoolinfo_min_mempool_feerate 7.2
        peerobserver_rpc_mempoolinfo_min_relay_tx_feerate 8.3
        peerobserver_rpc_mempoolinfo_transaction_count 1000
        peerobserver_rpc_mempoolinfo_transaction_fees 4000.1
        peerobserver_rpc_mempoolinfo_transaction_vbyte 2000
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_p2pextractor_ping_duration() {
    println!("test that p2p-extractor ping duration metrics work");

    publish_and_check(
        &[
            EventMsg::new(Event::P2pExtractorEvent(p2p_extractor::P2pExtractorEvent {
                event: Some(p2p_extractor::p2p_extractor_event::Event::PingDuration(
                    p2p_extractor::PingDuration { duration: 1234567 },
                )),
            }))
            .unwrap(),
        ],
        Subject::Validation,
        r#"
        peerobserver_p2pextractor_ping_duration_nanoseconds 1234567
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_logextractor_logevents() {
    println!("test that log-extractor log events metric work");

    publish_and_check(
        &[
            EventMsg::new(Event::LogExtractorEvent(log_extractor::LogEvent {
                category: LogDebugCategory::Unknown.into(),
                log_timestamp: 1234,
                event: Some(log_extractor::log_event::Event::UnknownLogMessage(
                    log_extractor::UnknownLogMessage {
                        raw_message: "test1".to_string(),
                    },
                )),
            }))
            .unwrap(),
            EventMsg::new(Event::LogExtractorEvent(log_extractor::LogEvent {
                category: LogDebugCategory::Unknown.into(),
                log_timestamp: 1234,
                event: Some(log_extractor::log_event::Event::UnknownLogMessage(
                    log_extractor::UnknownLogMessage {
                        raw_message: "test2".to_string(),
                    },
                )),
            }))
            .unwrap(),
        ],
        Subject::LogExtractor,
        r#"
        peerobserver_log_events 2
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_integration_metrics_logextractor_blockconnected_events() {
    println!("test that log-extractor block connected log events metric work");

    publish_and_check(
        &[
            EventMsg::new(Event::LogExtractorEvent(log_extractor::LogEvent {
                category: LogDebugCategory::Validation.into(),
                log_timestamp: 345,
                event: Some(log_extractor::log_event::Event::BlockConnectedLog(
                    log_extractor::BlockConnectedLog {
                        block_height: 1234,
                        block_hash:
                            "b00000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
                                .to_string(),
                    },
                )),
            }))
            .unwrap(),
            EventMsg::new(Event::LogExtractorEvent(log_extractor::LogEvent {
                category: LogDebugCategory::Validation.into(),
                log_timestamp: 3452,
                event: Some(log_extractor::log_event::Event::BlockConnectedLog(
                    log_extractor::BlockConnectedLog {
                        block_height: 2222,
                        block_hash:
                            "a00000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26d"
                                .to_string(),
                    },
                )),
            }))
            .unwrap(),
            EventMsg::new(Event::LogExtractorEvent(log_extractor::LogEvent {
                category: LogDebugCategory::Unknown.into(),
                log_timestamp: 1234,
                event: Some(log_extractor::log_event::Event::UnknownLogMessage(
                    log_extractor::UnknownLogMessage {
                        raw_message: "test2".to_string(),
                    },
                )),
            }))
            .unwrap(),
        ],
        Subject::LogExtractor,
        r#"
        peerobserver_log_block_connected_events 2
        peerobserver_log_events 3
        "#,
    )
    .await;
}
