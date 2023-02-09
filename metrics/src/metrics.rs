use lazy_static::lazy_static;
use prometheus::{self, HistogramVec, IntCounter, IntCounterVec, IntGauge};
use prometheus::{
    register_histogram_vec, register_int_counter, register_int_counter_vec, register_int_gauge,
    HistogramOpts, Opts,
};

// Prometheus Metrics

const NAMESPACE: &str = "networkobserver";

const SUBSYSTEM_RUNTIME: &str = "runtime";
const SUBSYSTEM_P2P: &str = "p2p";
const SUBSYSTEM_CONN: &str = "conn";

pub const LABEL_P2P_MSG_TYPE: &str = "message";
pub const LABEL_P2P_CONNECTION_TYPE: &str = "connection_type";
pub const LABEL_P2P_DIRECTION: &str = "direction";
pub const LABEL_P2P_SUBNET: &str = "subnet";

pub const LABEL_P2P_SERVICES: &str = "services";
pub const LABEL_P2P_ADDR_TIMESTAMP_OFFSET: &str = "timestamp_offset";
pub const LABEL_P2P_INV_TYPE: &str = "inv_type";
pub const LABEL_P2P_VERSION_USERAGENT: &str = "useragent";
pub const LABEL_P2P_FEEFILTER_FEERATE: &str = "feerate";
pub const LABEL_P2P_REJECT_REASON: &str = "rejectreason";
pub const LABEL_P2P_REJECT_COMMAND: &str = "rejectcommand";
pub const LABEL_P2P_REJECT_MESSAGE: &str = "rejectmessage";

pub const LABEL_CONN_NETWORK: &str = "network";
pub const LABEL_CONN_NETGROUP: &str = "netgroup";
pub const LABEL_CONN_ADDR: &str = "addr";
pub const LABEL_CONN_MISBEHAVING_SCORE_INC: &str = "score_inc";
pub const LABEL_CONN_MISBEHAVING_MESSAGE: &str = "missbehavingmessage";
pub const LABEL_CONN_MISBEHAVING_ID: &str = "id";

pub const BUCKETS_ADDR_ADDRESS_COUNT: [f64; 30] = [
    0f64, 1f64, 2f64, 3f64, 4f64, 5f64, 6f64, 7f64, 8f64, 9f64, 10f64, 15f64, 20f64, 25f64, 30f64,
    50f64, 75f64, 100f64, 150f64, 200f64, 250f64, 300f64, 400f64, 500f64, 600f64, 700f64, 800f64,
    900f64, 999f64, 1000f64,
];

pub const BUCKETS_INV_SIZE: [f64; 46] = [
    0f64, 1f64, 2f64, 3f64, 4f64, 5f64, 6f64, 7f64, 8f64, 9f64, 10f64, 15f64, 20f64, 25f64, 30f64,
    50f64, 75f64, 100f64, 150f64, 200f64, 250f64, 300f64, 400f64, 500f64, 600f64, 700f64, 800f64,
    900f64, 999f64, 1000f64, 2000f64, 3000f64, 4000f64, 5000f64, 6000f64, 7000f64, 8000f64,
    9000f64, 10_000f64, 20_000f64, 25_000f64, 30_000f64, 35_000f64, 40_000f64, 45_000f64,
    50_000f64,
];

pub const BUCKETS_ADDR_SERVICE_BITS: [f64; 32] = [
    0_f64,  // 0 NODE_NONE
    1_f64,  // 1 NODE_NETWORK
    2_f64,  // 2 NODE_GETUTXO
    3_f64,  // 4 NODE_BLOOM
    4_f64,  // 8 NODE_WITNESS
    5_f64,  // 16 NODE_XTHIN
    6_f64,  // 32
    7_f64,  // 64 NODE_COMPACT_FILTERS
    8_f64,  // 128
    9_f64,  // 256
    10_f64, // 512
    11_f64, // 1024 NODE_NETWORK_LIMITED
    12_f64, 13_f64, 14_f64, 15_f64, 16_f64, 17_f64, 18_f64, 19_f64, 20_f64, 21_f64, 22_f64, 23_f64,
    24_f64, 25_f64, 26_f64, 27_f64, 28_f64, 29_f64, 30_f64, 31_f64,
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

    /// Number of P2P network messages send or received by subnet.
    pub static ref P2P_MESSAGE_COUNT_BY_SUBNET: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("message_count_by_subnet", "Number of P2P network messages send or received by subnet.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION, LABEL_P2P_SUBNET]
    ).unwrap();

    /// Number of P2P network message bytes send or received.
    pub static ref P2P_MESSAGE_BYTES: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("message_bytes", "Number of P2P network messages bytes send or received.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_MSG_TYPE, LABEL_P2P_CONNECTION_TYPE, LABEL_P2P_DIRECTION]
    ).unwrap();

    /// Number of P2P network message bytes send or received by SUBNET.
    pub static ref P2P_MESSAGE_BYTES_BY_SUBNET: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("message_bytes_by_subnet", "Number of P2P network messages bytes send or received by SUBNET.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION, LABEL_P2P_SUBNET]
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

    /// Histogram of the service flags (per bit) of addresses contained in an "addr" message.
    pub static ref P2P_ADDR_SERVICES_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("addr_services_bits", "Histogram of the service flags (per bit) of addresses contained in an 'addr' message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P)
                .buckets(BUCKETS_ADDR_SERVICE_BITS.to_vec()),
            &[LABEL_P2P_DIRECTION]
        ).unwrap();

    /// Histogram of the service flags (per bit) of addresses contained in an "addrv2" message.
    pub static ref P2P_ADDRV2_SERVICES_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("addrv2_services_bits", "Histogram of the service flags (per bit) of addresses contained in an 'addrv2' message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P)
                .buckets(BUCKETS_ADDR_SERVICE_BITS.to_vec()),
            &[LABEL_P2P_DIRECTION]
        ).unwrap();

    /// Number of addresses with these service bits cointained in an 'addr' message.
    pub static ref P2P_ADDR_SERVICES: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("addr_services", "Number of addresses with these service bits cointained in an 'addr' message.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION, LABEL_P2P_SERVICES]
    ).unwrap();

    /// Number of addresses with these service bits cointained in an 'addrv2' message.
    pub static ref P2P_ADDRV2_SERVICES: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("addrv2_services", "Number of addresses with these service bits cointained in an 'addrv2' message.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION, LABEL_P2P_SERVICES]
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

    /// Number of empty addrv2 messages received and send (by address).
    pub static ref P2P_EMPTYADDRV2: IntCounterVec =
        register_int_counter_vec!(
            Opts::new("addrv2_empty", "Number of empty addrv2 messages received and send (by address).")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P),
            &[LABEL_P2P_DIRECTION, LABEL_CONN_ADDR]
        ).unwrap();

    // -------------------- Connections

    /// Number of inbound connections.
    pub static ref CONN_INBOUND: IntCounter =
    register_int_counter!(
        Opts::new("inbound", "Number of opnened inbound connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
    ).unwrap();

    /// Number of inbound connections by address.
    pub static ref CONN_INBOUND_ADDRESS: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("inbound_address", "Number of inbound connections by address.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_ADDR]
    ).unwrap();

    /// Number of inbound connections by subnet (where applicable).
    pub static ref CONN_INBOUND_SUBNET: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("inbound_subnet", "Number of inbound connections by subnet (where applicable).")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_P2P_SUBNET]
    ).unwrap();

    /// Number of inbound connections by network.
    pub static ref CONN_INBOUND_NETWORK: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("inbound_network", "Number of inbound connections by network.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_NETWORK]
    ).unwrap();

    /// Number of inbound connections by netgroup.
    pub static ref CONN_INBOUND_NETGROUP: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("inbound_netgroup", "Number of inbound connections by netgroup.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_NETGROUP]
    ).unwrap();

    /// Number of currently open inbound connections.
    pub static ref CONN_INBOUND_CURRENT: IntGauge =
    register_int_gauge!(
        Opts::new("inbound_current", "Number of currently open inbound connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
    ).unwrap();

    /// Number of outbound connections.
    pub static ref CONN_OUTBOUND: IntCounter =
    register_int_counter!(
        Opts::new("outbound", "Number of opened outbound connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
    ).unwrap();

    /// Number of currently open outbound connections.
    pub static ref CONN_OUTBOUND_CURRENT: IntGauge =
    register_int_gauge!(
        Opts::new("outbound_current", "Number of currently open outbound connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
    ).unwrap();

    /// Number of outbound connections by network.
    pub static ref CONN_OUTBOUND_NETWORK: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("outbound_network", "Number of opened outbound connections by network.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_NETWORK]
    ).unwrap();

    /// Number of outbound connections by netgroup.
    pub static ref CONN_OUTBOUND_NETGROUP: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("outbound_netgroup", "Number of opened outbound connections by netgroup.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_NETGROUP]
    ).unwrap();

    /// Number of closed connections.
    pub static ref CONN_CLOSED: IntCounter =
    register_int_counter!(
        Opts::new("closed", "Number of closed connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
    ).unwrap();

    /// Number of closed connections by address.
    pub static ref CONN_CLOSED_ADDRESS: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("closed_address", "Number of closed connections by address.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_ADDR]
    ).unwrap();

    /// Number of closed connections by network.
    pub static ref CONN_CLOSED_NETWORK: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("closed_network", "Number of closed connections by network.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_NETWORK]
    ).unwrap();

    /// Number of closed connections by netgroup.
    pub static ref CONN_CLOSED_NETGROUP: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("closed_netgroup", "Number of closed connections by netgroup.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_NETGROUP]
    ).unwrap();

    /// Number of evicted connections.
    pub static ref CONN_EVICTED: IntCounter =
    register_int_counter!(
        Opts::new("evicted", "Number of evicted connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN)
    ).unwrap();

    /// Number of evicted connections with information about their address and network.
    pub static ref CONN_EVICTED_WITHINFO: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("evicted_withinfo", "Number of evicted connections with information about their address and network.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_ADDR, LABEL_CONN_NETWORK]
    ).unwrap();

    /// Number of misbehaving connections.
    pub static ref CONN_MISBEHAVING: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("misbehaving", "Number of evicted connections.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_MISBEHAVING_ID, LABEL_CONN_MISBEHAVING_SCORE_INC, LABEL_CONN_MISBEHAVING_MESSAGE]
    ).unwrap();

    /// Score increase for misbehaving connections.
    pub static ref CONN_MISBEHAVING_SCORE_INC: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("misbehaving_score_increase", "Misbehaving score increase.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_CONN),
        &[LABEL_CONN_MISBEHAVING_ID, LABEL_CONN_MISBEHAVING_MESSAGE]
    ).unwrap();

    // -------------------- INVs

    /// Number of INV entries send and received with INV type.
    pub static ref P2P_INV_ENTRIES: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("inv_entries", "Number of INV entries send and received with INV type.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION, LABEL_P2P_INV_TYPE]
    ).unwrap();

    /// Histogram of the service flags (per bit) of addresses contained in an "addr" message.
    pub static ref P2P_INV_ENTRIES_HISTOGRAM: HistogramVec =
        register_histogram_vec!(
            HistogramOpts::new("inv_entries_histogram", "Histogram number of entries contained in an INV message.")
                .namespace(NAMESPACE)
                .subsystem(SUBSYSTEM_P2P)
                .buckets(BUCKETS_INV_SIZE.to_vec()),
            &[LABEL_P2P_DIRECTION]
        ).unwrap();

    /// Number of homogenous INV entries send and received with INV type.
    pub static ref P2P_INV_ENTRIES_HOMOGENOUS: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("invs_homogeneous", "Number of homogenous INV entries send and received.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION]
    ).unwrap();

    /// Number of heterogeneous INV entries send and received with INV type.
    pub static ref P2P_INV_ENTRIES_HETEROGENEOUS: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("invs_heterogeneous", "Number of heterogenous INVs send and received.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION]
    ).unwrap();

    // -------------------- Pings

    /// Number of Pings received by address
    pub static ref P2P_PING_ADDRESS: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("ping_address", "Number of Pings received by address.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_CONN_ADDR]
    ).unwrap();

    /// Number of Pings received by subnet (where applicable)
    pub static ref P2P_PING_SUBNET: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("ping_subnet", "Number of Pings received by subnet (where applicable).")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_SUBNET]
    ).unwrap();

    /// Number of "old" pings (without a value) received by address
    pub static ref P2P_OLDPING_ADDRESS: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("oldping_address", "Number of 'old' Pings (without a value) received by address.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_CONN_ADDR]
    ).unwrap();

    // -------------------- Version

    /// Number of version messages received by address
    pub static ref P2P_VERSION_ADDRESS: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("version_address", "Number of version messags received by address.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_CONN_ADDR]
    ).unwrap();

    /// Number of version messages received by user_agent
    pub static ref P2P_VERSION_USERAGENT: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("version_useragent", "Number of version messags received by useragent.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_VERSION_USERAGENT]
    ).unwrap();

    // -------------------- Feefilter

    /// Number of feefilter messages received and sent by feerate
    pub static ref P2P_FEEFILTER_FEERATE: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("feefilter_feerate", "Number of feefilter messages received and sent by feerate.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_DIRECTION, LABEL_P2P_FEEFILTER_FEERATE]
    ).unwrap();

    // -------------------- Reject

    /// Number of reject messages received by addr, command and rejected command
    pub static ref P2P_REJECT_ADDR: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("reject_addr", "Number of reject messages received by addr, command and reason.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_CONN_ADDR, LABEL_P2P_REJECT_COMMAND, LABEL_P2P_REJECT_REASON]
    ).unwrap();


    /// Number of reject messages received by command, reason and message
    pub static ref P2P_REJECT_MESSAGE: IntCounterVec =
    register_int_counter_vec!(
        Opts::new("reject_message", "Number of reject messages received by command, reason and message.")
            .namespace(NAMESPACE)
            .subsystem(SUBSYSTEM_P2P),
        &[LABEL_P2P_REJECT_COMMAND, LABEL_P2P_REJECT_REASON, LABEL_P2P_REJECT_MESSAGE]
    ).unwrap();
}
