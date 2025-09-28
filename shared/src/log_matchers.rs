use crate::protobuf::log_extractor::log_event::Event;
use crate::protobuf::log_extractor::{BlockConnectedLog, LogEvent, UnknownLogMessage};
use chrono::DateTime;
use lazy_static::lazy_static;
use regex::Regex;
use std::fmt;
use std::str::FromStr;

pub enum LogDebugCategory {
    Unknown,
    Addrman,
    Bench,
    Blockstorage,
    Cmpctblock,
    Coindb,
    Estimatefee,
    Http,
    I2p,
    Ipc,
    Leveldb,
    Libevent,
    Mempool,
    Mempoolrej,
    Net,
    Proxy,
    Prune,
    Qt,
    Rand,
    Reindex,
    Rpc,
    Scan,
    Selectcoins,
    Tor,
    Txpackages,
    Txreconciliation,
    Validation,
    Walletdb,
    Zmq,
}

impl FromStr for LogDebugCategory {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "addrman" => Ok(LogDebugCategory::Addrman),
            "bench" => Ok(LogDebugCategory::Bench),
            "blockstorage" => Ok(LogDebugCategory::Blockstorage),
            "cmpctblock" => Ok(LogDebugCategory::Cmpctblock),
            "coindb" => Ok(LogDebugCategory::Coindb),
            "estimatefee" => Ok(LogDebugCategory::Estimatefee),
            "http" => Ok(LogDebugCategory::Http),
            "i2p" => Ok(LogDebugCategory::I2p),
            "ipc" => Ok(LogDebugCategory::Ipc),
            "leveldb" => Ok(LogDebugCategory::Leveldb),
            "libevent" => Ok(LogDebugCategory::Libevent),
            "mempool" => Ok(LogDebugCategory::Mempool),
            "mempoolrej" => Ok(LogDebugCategory::Mempoolrej),
            "net" => Ok(LogDebugCategory::Net),
            "proxy" => Ok(LogDebugCategory::Proxy),
            "prune" => Ok(LogDebugCategory::Prune),
            "qt" => Ok(LogDebugCategory::Qt),
            "rand" => Ok(LogDebugCategory::Rand),
            "reindex" => Ok(LogDebugCategory::Reindex),
            "rpc" => Ok(LogDebugCategory::Rpc),
            "scan" => Ok(LogDebugCategory::Scan),
            "selectcoins" => Ok(LogDebugCategory::Selectcoins),
            "tor" => Ok(LogDebugCategory::Tor),
            "txpackages" => Ok(LogDebugCategory::Txpackages),
            "txreconciliation" => Ok(LogDebugCategory::Txreconciliation),
            "validation" => Ok(LogDebugCategory::Validation),
            "walletdb" => Ok(LogDebugCategory::Walletdb),
            "zmq" => Ok(LogDebugCategory::Zmq),
            _ => Ok(LogDebugCategory::Unknown),
        }
    }
}

impl fmt::Display for LogDebugCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            LogDebugCategory::Addrman => "addrman",
            LogDebugCategory::Bench => "bench",
            LogDebugCategory::Blockstorage => "blockstorage",
            LogDebugCategory::Cmpctblock => "cmpctblock",
            LogDebugCategory::Coindb => "coindb",
            LogDebugCategory::Estimatefee => "estimatefee",
            LogDebugCategory::Http => "http",
            LogDebugCategory::I2p => "i2p",
            LogDebugCategory::Ipc => "ipc",
            LogDebugCategory::Leveldb => "leveldb",
            LogDebugCategory::Libevent => "libevent",
            LogDebugCategory::Mempool => "mempool",
            LogDebugCategory::Mempoolrej => "mempoolrej",
            LogDebugCategory::Net => "net",
            LogDebugCategory::Proxy => "proxy",
            LogDebugCategory::Prune => "prune",
            LogDebugCategory::Qt => "qt",
            LogDebugCategory::Rand => "rand",
            LogDebugCategory::Reindex => "reindex",
            LogDebugCategory::Rpc => "rpc",
            LogDebugCategory::Scan => "scan",
            LogDebugCategory::Selectcoins => "selectcoins",
            LogDebugCategory::Tor => "tor",
            LogDebugCategory::Txpackages => "txpackages",
            LogDebugCategory::Txreconciliation => "txreconciliation",
            LogDebugCategory::Validation => "validation",
            LogDebugCategory::Walletdb => "walletdb",
            LogDebugCategory::Zmq => "zmq",
            LogDebugCategory::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

trait LogMatcher {
    fn parse_event(line: &str) -> Option<Event>;
}

impl LogMatcher for UnknownLogMessage {
    fn parse_event(line: &str) -> Option<Event> {
        Some(Event::UnknownLogMessage(UnknownLogMessage {
            raw_message: line.to_string(),
        }))
    }
}

impl LogMatcher for BlockConnectedLog {
    // 2025-09-27T01:52:01Z [validation] Enqueuing BlockConnected: block hash=41109f31c8ca4d8683ab5571ba462292ddb8486dee6ecd2e62901accc7952f0b block height=437
    fn parse_event(line: &str) -> Option<Event> {
        let Some(caps) = BLOCK_CONNECTED_REGEX.captures(&line) else {
            return None;
        };

        let block_hash = caps.get(1)?.as_str().to_string();
        let block_height = caps.get(2)?.as_str().parse::<u32>().ok()?;
        Some(Event::BlockConnectedLog(BlockConnectedLog {
            block_hash,
            block_height,
        }))
    }
}

lazy_static! {
    static ref LOG_LINE_REGEX: Regex = Regex::new(r"^([^ ]+)\s+(?:\[([^\]]+)\]\s+)?(.+)$").unwrap();
    static ref BLOCK_CONNECTED_REGEX: Regex =
        Regex::new(r"Enqueuing BlockConnected: block hash=([0-9a-fA-F]+) block height=(\d+)")
            .unwrap();
}

pub fn parse_log_event(line: &str) -> LogEvent {
    let (timestamp, category, message) = parse_common_log_data(line);

    let matchers: Vec<fn(&str) -> Option<Event>> = vec![BlockConnectedLog::parse_event];
    for matcher in &matchers {
        if let Some(event) = matcher(&message) {
            return LogEvent {
                timestamp,
                category: category.to_string(),
                event: Some(event),
            };
        }
    }

    // if no matcher succeeds, return unknown
    LogEvent {
        timestamp,
        category: category.to_string(),
        event: Some(Event::UnknownLogMessage(UnknownLogMessage {
            raw_message: message,
        })),
    }
}

fn parse_common_log_data(line: &str) -> (u64, LogDebugCategory, String) {
    let re = Regex::new(r"^([^ ]+)\s+(?:\[([^\]]+)\]\s+)?(.+)$").unwrap();
    let caps = re.captures(line);
    if caps.is_none() {
        return (0, LogDebugCategory::Unknown, String::new());
    }

    let caps = caps.unwrap();
    let timestamp_str = &caps[1];
    let category = caps.get(2).map(|m| m.as_str());
    let timestamp = match DateTime::parse_from_rfc3339(timestamp_str) {
        Ok(dt) => dt.timestamp() as u64,
        Err(_) => 0,
    };
    let log_type = match category.and_then(|cat| LogDebugCategory::from_str(cat).ok()) {
        Some(cat) => cat,
        None => LogDebugCategory::Unknown,
    };

    (timestamp, log_type, caps[3].to_string())
}

// TODO: mempool_event::Event::Added
// TODO: mempool_event::Event::Removed
// TODO: mempool_event::Event::Replaced
// TODO: mempool_event::Event::Rejected
// TODO: validation_event::Event::BlockConnected
// TODO: connection_event::Event::Inbound
// TODO: connection_event::Event::Outbound
// TODO: connection_event::Event::Closed
// TODO: connection_event::Event::InboundEvicted
// TODO: connection_event::Event::Misbehaving
// TODO: addrman_event::Event::New
// TODO: addrman_event::Event::Tried
// TODO: p2p_extractor_event::Event::PingDuration
// TODO: log_event::Event::UnknownLogMessage
// TODO: rpc_event::Event::PeerInfos
// TODO: net_msg::message::Msg::Addr
// TODO: net_msg::message::Msg::Addrv2
// TODO: net_msg::message::Msg::Emptyaddrv2
// TODO: net_msg::message::Msg::Inv
// TODO: net_msg::message::Msg::Ping
// TODO: net_msg::message::Msg::Oldping
// TODO: net_msg::message::Msg::Version
// TODO: net_msg::message::Msg::Feefilter
// TODO: net_msg::message::Msg::Reject
