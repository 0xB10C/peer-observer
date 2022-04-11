use lazy_static::lazy_static;
use prometheus::{self, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge};
use prometheus::{
    register_histogram, register_histogram_vec, register_int_counter, register_int_counter_vec,
    register_int_gauge, HistogramOpts, Opts,
};

// Prometheus Metrics

const NAMESPACE: &str = "networkobserver";

const SUBSYSTEM_RUNTIME: &str = "runtime";
const SUBSYSTEM_P2P: &str = "p2p";

pub const LABEL_P2P_MSG_TYPE: &str = "message";
pub const LABEL_P2P_CONNECTION_TYPE: &str = "connection_type";
pub const LABEL_P2P_DIRECTION: &str = "direction";

pub const LABEL_P2P_ADDR_TIMESTAMP_OFFSET: &str = "timestamp_offset";

pub const BUCKETS_ADDR_ADDRESS_COUNT: [f64; 30] = [
    0f64, 1f64, 2f64, 3f64, 4f64, 5f64, 6f64, 7f64, 8f64, 9f64, 10f64, 15f64, 20f64, 25f64, 30f64,
    50f64, 75f64, 100f64, 150f64, 200f64, 250f64, 300f64, 400f64, 500f64, 600f64, 700f64, 800f64,
    900f64, 1000f64, 999f64,
];

// Buckets for addr(v2) message timestamp offset in seconds.
pub const BUCKETS_ADDR_ADDRESS_TIMESTAMP_OFFSET: [f64; 26] = [
    0f64,
    1f64,
    2f64,
    4f64,
    8f64,
    16f64,
    32f64,
    64f64,
    128f64,
    256f64,
    512f64,
    1024f64,
    2048f64,
    4096f64,
    8192f64,
    16384f64,
    32768f64,
    65536f64,
    131072f64,
    262144f64,
    524288f64,
    1048576f64,
    2097152f64,
    4194304f64,
    8388608f64,
    16777216f64,
];

lazy_static! {

    // -------------------- Runtime

    /// UNIX epoch timestamp of bitcoind-observer start. Can be used to alert on
    /// bitcoind-observer restarts.
    pub static ref RUNTIME_START_TIMESTAMP: IntGauge =
        register_int_gauge!(
            Opts::new("start_timestamp", "UNIX epoch timestamp of bitcoind-observer start")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_RUNTIME)
        ).unwrap();

    // -------------------- General

    /// Number of P2P network messages send or received.
    pub static ref P2P_MESSAGE_COUNT: IntCounterVec =
        register_int_counter_vec!(
            Opts::new("message_count", "Number of P2P network messages send or received.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P),
            &[LABEL_P2P_MSG_TYPE, LABEL_P2P_CONNECTION_TYPE, LABEL_P2P_DIRECTION]
        ).unwrap();

    /// Number of P2P network messages bytes send or received.
    pub static ref P2P_MESSAGE_BYTES: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("message_bytes", "Number of P2P network messages bytes send or received.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_MSG_TYPE, LABEL_P2P_CONNECTION_TYPE, LABEL_P2P_DIRECTION]
    ).unwrap();

    // -------------------- Addr


    /// Histogram of the number of addresses contained in an "addr" message.
    pub static ref P2P_ADDR_ADDRESS_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("addr_addresses", "Histogram of the number of addresses contained in an outbound 'addr' message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P)
                .buckets(BUCKETS_ADDR_ADDRESS_COUNT.to_vec()),
            &[LABEL_P2P_DIRECTION]
        ).unwrap();

    /// Histogram of the timestamp offset (in seconds) of addresses contained in an "addr" message.
    pub static ref P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("addr_timestamp_offset_seconds", "Histogram of the timestamp offset (in seconds) of addresses contained in an 'addr' message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P)
                .buckets(BUCKETS_ADDR_ADDRESS_TIMESTAMP_OFFSET.to_vec()),
            &[LABEL_P2P_DIRECTION, LABEL_P2P_ADDR_TIMESTAMP_OFFSET]
        ).unwrap();

    /// Histogram of the number of addresses contained in an "addrv2" message.
    pub static ref P2P_ADDRV2_ADDRESS_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("addrv2_addresses", "Histogram of the number of addresses contained in an 'addrv2' message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P)
                .buckets(BUCKETS_ADDR_ADDRESS_COUNT.to_vec()),
            &[LABEL_P2P_DIRECTION]
        ).unwrap();

    /// Histogram of the timestamp offset (in seconds) of addresses contained in an "addrv2" message.
    pub static ref P2P_ADDRV2_TIMESTAMP_OFFSET_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("addrv2_timestamp_offset_seconds", "Histogram of the timestamp offset (in seconds) of addresses contained in an 'addrv2' message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P)
                .buckets(BUCKETS_ADDR_ADDRESS_TIMESTAMP_OFFSET.to_vec()),
            &[LABEL_P2P_DIRECTION, LABEL_P2P_ADDR_TIMESTAMP_OFFSET]
        ).unwrap();
}
