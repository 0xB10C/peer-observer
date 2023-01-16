use lazy_static::lazy_static;
use prometheus::{self, HistogramVec, IntCounter, IntCounterVec, IntGauge};
use prometheus::{
    register_histogram_vec, register_int_counter, register_int_counter_vec, register_int_gauge,
    HistogramOpts, Opts,
};

// Prometheus Metrics

const NAMESPACE: &str = "connectivitycheck";

const SUBSYSTEM_RUNTIME: &str = "runtime";
const SUBSYSTEM_ADDR: &str = "addr";

pub const LABEL_NETWORK: &str = "network";
pub const LABEL_ADDR_VERSION: &str = "addr_version";

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

    /// Number of successful connections.
    pub static ref ADDR_SUCCESSFUL_CONNECTION: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("successful_connection", "Number of successful connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_ADDR),
        &[LABEL_NETWORK, LABEL_ADDR_VERSION]
    ).unwrap();





}
