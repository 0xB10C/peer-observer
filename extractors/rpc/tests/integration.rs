#![cfg(feature = "nats_integration_tests")]
#![cfg(feature = "node_integration_tests")]

use shared::{
    async_nats,
    corepc_node,
    event_msg::{EventMsg, event_msg::Event},
    futures::StreamExt,
    log::{self, info},
    nats_server_for_testing::NatsServerForTesting,
    prost::Message,
    rand::{self, Rng},
    rpc::rpc_event::Event::PeerInfos,
    simple_logger::SimpleLogger,
    tokio::{self, sync::watch},
};

use std::{
    sync::{
        Once, OnceLock,
        atomic::{AtomicU16, Ordering},
    },
};

use rpc_extractor::Args;

static INIT: Once = Once::new();
static NEXT_NATS_PORT: OnceLock<AtomicU16> = OnceLock::new();

// 1 second query interval for fast tests
const QUERY_INTERVAL_SECONDS: u64 = 1;

fn setup() -> u16 {
    INIT.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Trace)
            .init()
            .unwrap();

        let mut rng = rand::rng();

        // choose start ports from the ephemeral port range
        let nats_start = rng.random_range(49152..65500);
        NEXT_NATS_PORT.set(AtomicU16::new(nats_start)).unwrap();
    });
    let nats_port = NEXT_NATS_PORT.get().unwrap().fetch_add(1, Ordering::SeqCst);
    nats_port
}

fn make_test_args(nats_port: u16, rpc_url: String, cookie_file: String) -> Args {
    Args::new(
        format!("127.0.0.1:{}", nats_port),
        log::Level::Trace,
        rpc_url,
        cookie_file,
        QUERY_INTERVAL_SECONDS,
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

async fn check(_test_getpeerinfo: bool, check_expected: fn(Event) -> ()) {
    let (node1, _node2) = setup_two_connected_nodes();
    let nats_port = setup();
    let _nats_server = NatsServerForTesting::new(nats_port).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let rpc_extractor_handle = tokio::spawn(async move {
        let args = make_test_args(
            nats_port,
            node1.rpc_url().replace("http://", ""),
            node1.params.cookie_file.display().to_string(),
        );
        rpc_extractor::run(args, shutdown_rx.clone())
            .await
            .expect("rpc extractor failed");
    });

    let nc = async_nats::connect(format!("127.0.0.1:{}", nats_port))
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
    rpc_extractor_handle.await.unwrap();
}


#[tokio::test]
async fn test_integration_rpc_getpeerinfo2() {
    println!("test that we receive getpeerinfo RPC events");

    check(true, |event| {
        match event {
            Event::Rpc(r) => {
                if let Some(ref e) = r.event {
                    match e {
                        PeerInfos(p) => {
                            // we expect 1 peer to be connected
                            assert_eq!(p.infos.len(), 1);
                            let peer = p.infos.first().expect("we have expactly one peer here");
                            assert_eq!(peer.connection_type, "inbound");

                            return;
                        }
                        // TODO: once we have more RPCs, we are going to need this.
                        //_ => panic!("unexpected RPC data {:?}", r.event),
                    }
                }
            }
            _ => panic!("unexpected event {:?}", event),
        }
    }).await;
}
