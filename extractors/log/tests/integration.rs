#![cfg(feature = "nats_integration_tests")]
#![cfg(feature = "node_integration_tests")]

use shared::{
    async_nats, corepc_node,
    futures::StreamExt,
    log::{self, info},
    prost::Message,
    protobuf::{
        event_msg::{EventMsg, event_msg::Event},
        log_extractor::log_event,
    },
    simple_logger::SimpleLogger,
    testing::nats_server::NatsServerForTesting,
    tokio::{self, sync::watch},
};

use std::{sync::Once};

use log_extractor::Args;

static INIT: Once = Once::new();

fn setup() -> () {
    INIT.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Trace)
            .init()
            .unwrap();
    });
}

fn spawn_pipe(log_path: String, pipe_path: String) -> () {
    // Create pipe
    std::process::Command::new("mkfifo")
        .arg(&pipe_path)
        .status()
        .expect("Failed to create named pipe");
    log::info!("Created named pipe at {}", &pipe_path);

    // Start tail -F from debug.log to the pipe
    log::info!("Running: bash -c 'tail -F {} > {}'", log_path, pipe_path);
    tokio::process::Command::new("bash")
        .arg("-c")
        .arg(format!("tail -F {} > {}", log_path, pipe_path))
        .spawn()
        .expect("Failed to spawn tail");
}

fn make_test_args(nats_port: u16, bitcoind_pipe: String) -> Args {
    Args::new(
        format!("127.0.0.1:{}", nats_port),
        bitcoind_pipe,
        log::Level::Trace,
    )
}

fn setup_node(conf: corepc_node::Conf) -> corepc_node::Node {
    info!("env BITCOIND_EXE={:?}", std::env::var("BITCOIND_EXE"));
    info!("exe_path={:?}", corepc_node::exe_path());

    if let Ok(exe_path) = corepc_node::exe_path() {
        info!("Using bitcoind at '{}'", exe_path);
        return corepc_node::Node::with_conf(exe_path, &conf).unwrap();
    }

    info!("Trying to download a bitcoind..");
    return corepc_node::Node::from_downloaded_with_conf(&conf).unwrap();
}

fn setup_two_connected_nodes() -> (corepc_node::Node, corepc_node::Node) {
    // node1 listens for p2p connections
    let mut node1_conf = corepc_node::Conf::default();
    node1_conf.p2p = corepc_node::P2P::Yes;
    let node1 = setup_node(node1_conf);

    // node2 connects to node1
    let mut node2_conf = corepc_node::Conf::default();
    node2_conf.p2p = node1.p2p_connect(true).unwrap();
    let node2 = setup_node(node2_conf);

    (node1, node2)
}

async fn check(check_expected: fn(Event) -> ()) {
    setup();
    let (node1, _node2) = setup_two_connected_nodes();
    let nats_server = NatsServerForTesting::new().await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let node1_workdir = node1.workdir().to_str().unwrap().to_string();

    let log_extractor_handle = tokio::spawn(async move {
        let log_path = format!("{}/regtest/debug.log", node1_workdir);
        let pipe_path = format!("{}/bitcoind_pipe", node1_workdir);
        let _tail_handle = spawn_pipe(log_path, pipe_path.clone());

        // Wait a bit for tail to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let args = make_test_args(nats_server.port, pipe_path.to_string());

        log_extractor::run(args, shutdown_rx.clone())
            .await
            .expect("log extractor failed");
    });

    let nc = async_nats::connect(format!("127.0.0.1:{}", nats_server.port))
        .await
        .unwrap();
    let mut sub = nc.subscribe("*").await.unwrap();

    while let Some(msg) = sub.next().await {
        let unwrapped = EventMsg::decode(msg.payload).unwrap();
        if let Some(event) = unwrapped.event {
            check_expected(event);
            break;
        }
    }

    shutdown_tx.send(true).unwrap();
    log_extractor_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_log() {
    println!("test that we receive log events");

    check(|event| match event {
        Event::LogExtractorEvent(r) => {
            if let Some(ref e) = r.event {
                match e {
                    log_event::Event::UnknownLogMessage(unknown_log_message) => println!(
                        "Received unknown log message: {}",
                        unknown_log_message.raw_message
                    ),
                    log_event::Event::BlockConnectedLog(block_connected_log) => println!(
                        "Received BlockConnectedLog: block_hash={}, block_height={}",
                        block_connected_log.block_hash, block_connected_log.block_height
                    ),
                }
            }
        }
        _ => panic!("unexpected event {:?}", event),
    })
    .await;
}
