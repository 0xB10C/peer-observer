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

pub const LABEL_CONN_NETWORK: &str = "network";
pub const LABEL_CONN_NETGROUP: &str = "netgroup";
pub const LABEL_CONN_ADDR: &str = "addr";
pub const LABEL_CONN_MISBEHAVING_SCORE_INC: &str = "score_inc";
pub const LABEL_CONN_MISBEHAVING_MESSAGE: &str = "missbehavingmessage";
pub const LABEL_CONN_MISBEHAVING_ID: &str = "id";

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
    pub static ref ADDR_PROCESSED: IntCounter =
        register_int_counter!(
            Opts::new("processed", "Number of addresses processed.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_ADDR),
        ).unwrap();

    /// Number of successful handshakes.
    pub static ref ADDR_SUCCESSFUL_HANDSHAKES: IntCounter =
    register_int_counter!(
        Opts::new("successful_handshakes", "/// Number of successful handshakes.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_ADDR),
    ).unwrap();

}
