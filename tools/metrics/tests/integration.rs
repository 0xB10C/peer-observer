#![cfg(feature = "nats_integration_tests")]

use metrics::error::RuntimeError;
use metrics::Args;

use shared::{
    addrman::{self, InsertNew, InsertTried},
    event_msg::{event_msg::Event, EventMsg},
    log,
    mempool::{self, Added, Rejected, Removed, Replaced},
    nats_publisher_for_testing::NatsPublisherForTesting,
    nats_server_for_testing::NatsServerForTesting,
    nats_subjects::Subject,
    net_conn::{self, Connection},
    net_msg::{
        self, message::Msg, Addr, AddrV2, FeeFilter, Inv, Metadata, Ping, Pong, Reject, Version,
    },
    primitive::{self, inventory_item::Item, Address, InventoryItem},
    prost::Message,
    rand::{self, Rng},
    rpc::{self, PeerInfo, PeerInfos},
    simple_logger::SimpleLogger,
    tokio::{self, sync::watch, time::sleep},
    util::current_timestamp,
    validation::{self, BlockConnected},
};

use std::{
    collections::HashMap,
    io::ErrorKind,
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
    let (nats_port, mut metrics_port) = setup();
    let _nats_server = NatsServerForTesting::new(nats_port).await;
    let nats_publisher = NatsPublisherForTesting::new(nats_port).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics_handle = tokio::spawn(async move {
        loop {
            let args = make_test_args(nats_port, metrics_port);
            match metrics::run(args, shutdown_rx.clone()).await {
                Ok(_) => break,
                Err(e) => match e {
                    RuntimeError::Io(e) => match e.kind() {
                        ErrorKind::AddrInUse => {
                            let new_port = NEXT_METRICS_PORT
                                .get()
                                .unwrap()
                                .fetch_add(1, Ordering::SeqCst);
                            println!(
                                "Port {} seems to be already in use. Trying port {} next..",
                                metrics_port, new_port
                            );
                            metrics_port = new_port;
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
        println!("publishing: {:?}", event);
        nats_publisher
            .publish(subject.to_string(), event.encode_to_vec())
            .await;
    }

    sleep(Duration::from_millis(100)).await;

    let expected_lines: Vec<&str> = expected.split('\n').collect();
    assert!(check_metrics(metrics_port, &expected_lines).expect("Could not fetch metrics"));

    shutdown_tx.send(true).unwrap();
    metrics_handle.await.unwrap();
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
        peerobserver_p2p_ping_subnet{subnet="127.0.0.0"} 1
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
        peerobserver_p2p_ping_subnet{subnet="162.218.65.0"} 1
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
        &[EventMsg::new(Event::Msg(net_msg::Message {
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
        .unwrap()],
        Subject::NetMsg,
        r#"
        peerobserver_p2p_message_bytes{connection_type="2",direction="inbound",message="version"} 2
        peerobserver_p2p_message_count{connection_type="2",direction="inbound",message="version"} 1
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
        .unwrap()],
        Subject::NetConn,
        r#"
        peerobserver_conn_inbound 1
        peerobserver_conn_inbound_current 123
        peerobserver_conn_inbound_network{network="2"} 1
        peerobserver_conn_inbound_subnet{subnet="127.0.0.0"} 1
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
        .unwrap()],
        Subject::NetConn,
        r#"
        peerobserver_conn_inbound 0
        peerobserver_conn_outbound 1
        peerobserver_conn_outbound_current 321
        erobserver_conn_outbound_network{network="3"} 1
        peerobserver_conn_outbound_subnet{subnet="1.1.1.0"} 1
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
                shared::net_conn::ClosedConnection {
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
                shared::net_conn::EvictedInboundConnection {
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
        peerobserver_conn_evicted_inbound_withinfo{addr="2.2.2.2",network="2"} 1
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
                shared::net_conn::MisbehavingConnection {
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
                    shared::net_conn::InboundConnection {
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
                    shared::net_conn::InboundConnection {
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
        peerobserver_conn_inbound_banlist_gmax{addr="162.218.65.123"} 1
        peerobserver_conn_inbound_banlist_monero{addr="162.218.65.123"} 1
        peerobserver_conn_inbound_current 2
        peerobserver_conn_inbound_network{network="2"} 2
        peerobserver_conn_inbound_subnet{subnet="162.218.65.0"} 1
        peerobserver_conn_inbound_subnet{subnet="179.43.182.0"} 1
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
