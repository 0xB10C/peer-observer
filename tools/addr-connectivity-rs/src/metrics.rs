use lazy_static::lazy_static;
use prometheus::{self, HistogramVec, IntCounterVec, IntGauge};
use prometheus::{
    register_histogram_vec, register_int_counter_vec, register_int_gauge, HistogramOpts, Opts,
};

// Prometheus Metrics

const NAMESPACE: &str = "connectivitycheck";

const SUBSYSTEM_RUNTIME: &str = "runtime";
const SUBSYSTEM_ADDR: &str = "addr";

pub const LABEL_NETWORK: &str = "network";
pub const LABEL_SUCCESSFUL: &str = "successful";
pub const LABEL_ADDR_VERSION: &str = "addr_version";
pub const LABEL_SOURCE_IP: &str = "source_ip";
pub const LABEL_ADDR_TIMESTAMP_OFFSET: &str = "timestamp_offset";

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

    /// Number of addresses processed.
    pub static ref ADDR_TRIED: IntCounterVec =
        register_int_counter_vec!(
            Opts::new("tried", "Number of addresses tried.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_ADDR),
        &[LABEL_NETWORK, LABEL_ADDR_VERSION]
    ).unwrap();

    /// Number of cached addresses.
    pub static ref ADDR_CACHED: IntCounterVec =
        register_int_counter_vec!(
            Opts::new("cached", "Number of addresses cached.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_ADDR),
        &[LABEL_NETWORK, LABEL_ADDR_VERSION]
    ).unwrap();

    /// Number of successful connections.
    pub static ref ADDR_SUCCESSFUL_CONNECTION: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("successful_connection", "Number of successful connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_ADDR),
        &[LABEL_NETWORK, LABEL_ADDR_VERSION, LABEL_SOURCE_IP]
    ).unwrap();

    /// Number of unsuccessful connections by source.
    pub static ref ADDR_UNSUCCESSFUL_CONNECTION: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("unsuccessful_connection", "Number of unsuccessful connections by source.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_ADDR),
        &[LABEL_NETWORK, LABEL_ADDR_VERSION, LABEL_SOURCE_IP]
    ).unwrap();

    /// Histogram of the timestamp offset (in seconds) of addresses contained in an "addr" message.
    pub static ref P2P_ADDR_TIMESTAMP_OFFSET_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("addr_timestamp_offset_seconds", "Histogram of the timestamp offset (in seconds) of addresses contained in an 'addr' message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_ADDR)
                .buckets(BUCKETS_ADDR_ADDRESS_TIMESTAMP_OFFSET.to_vec()),
            &[LABEL_NETWORK, LABEL_ADDR_VERSION, LABEL_ADDR_TIMESTAMP_OFFSET, LABEL_SUCCESSFUL]
        ).unwrap();

}
