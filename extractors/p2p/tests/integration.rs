#![cfg(feature = "nats_integration_tests")]
#![cfg(feature = "node_integration_tests")]

use shared::{
    async_nats, corepc_node,
    futures::StreamExt,
    log::{self, info},
    prost::Message,
    protobuf::event_msg::{EventMsg, event_msg::Event},
    protobuf::p2p_extractor::p2p_extractor_event::Event::PingDuration,
    rand::{self, Rng},
    simple_logger::SimpleLogger,
    testing::nats_server::NatsServerForTesting,
    tokio::{
        self,
        sync::watch,
        time::{Duration, sleep},
    },
};

use std::sync::{
    Once, OnceLock,
    atomic::{AtomicU16, Ordering},
};

use p2p_extractor::{Args, Network};

static INIT: Once = Once::new();
static NEXT_P2PEXTRACTOR_PORT: OnceLock<AtomicU16> = OnceLock::new();

// 1 second ping interval for fast tests
const PING_INTERVAL_SECONDS: u64 = 1;

fn setup() -> u16 {
    INIT.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Trace)
            .init()
            .unwrap();

        let mut rng = rand::rng();

        // choose start ports from the ephemeral port range
        let p2p_extractor_start = rng.random_range(49152..65500);
        NEXT_P2PEXTRACTOR_PORT
            .set(AtomicU16::new(p2p_extractor_start))
            .unwrap();
    });
    let p2p_extractor_port = NEXT_P2PEXTRACTOR_PORT
        .get()
        .unwrap()
        .fetch_add(1, Ordering::SeqCst);
    p2p_extractor_port
}

fn make_test_args(nats_port: u16, p2p_address: String, disable_ping: bool) -> Args {
    Args::new(
        format!("127.0.0.1:{}", nats_port),
        log::Level::Trace,
        p2p_address,
        Network::Regtest,
        PING_INTERVAL_SECONDS,
        disable_ping,
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

fn setup_node_with_addnode(p2p_extractor_port: u16) -> corepc_node::Node {
    let mut node_conf = corepc_node::Conf::default();
    let addnode = format!("--addnode=127.0.0.1:{}", p2p_extractor_port);
    node_conf.args = vec!["--regtest", "--debug=net", &addnode];
    // enabling this is useful for debugging, but enabling this by default will
    // be quite spammy.
    node_conf.view_stdout = false;
    let node = setup_node(node_conf);
    node
}

async fn check(disable_ping: bool, check_expected: fn(Event) -> ()) {
    let p2p_extractor_port = setup();
    let nats_server = NatsServerForTesting::new().await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let p2p_extractor_handle = tokio::spawn(async move {
        let args = make_test_args(
            nats_server.port,
            format!("127.0.0.1:{}", p2p_extractor_port),
            disable_ping,
        );
        p2p_extractor::run(args, shutdown_rx.clone())
            .await
            .expect("p2p-extractor failed");
    });

    // allow the p2p-extractor to start
    sleep(Duration::from_secs(2)).await;

    // only start the node after the P2P extractor to make sure the
    // extractor is ready to accept connections
    let _node = setup_node_with_addnode(p2p_extractor_port);

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
    p2p_extractor_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_p2pextractor_ping_measurements() {
    println!("test that we receive Ping measurement P2P-extractor events");

    check(false, |event| {
        match event {
            Event::P2pExtractorEvent(p) => {
                if let Some(ref e) = p.event {
                    match e {
                        PingDuration(p) => {
                            // we expect the duration to be >0ns
                            assert!(p.duration > 0);
                            return;
                        } // TODO: once we have more P2P extractor events, we are going to need this.
                          //_ => panic!("unexpected P2P extractor event {:?}", p.event),
                    }
                }
            }
            _ => panic!("unexpected event {:?}", event),
        }
    })
    .await;
}
