use crate::protobuf::log_extractor::log_event::Event;
use crate::protobuf::log_extractor::{
    BlockConnectedLog, LogDebugCategory, LogEvent, UnknownLogMessage,
};
use lazy_static::lazy_static;
use regex::Regex;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

static BLOCK_HASH_PATTERN: &str = r"[0-9a-f]{64}";
static ISO8601_DATE_REGEX: &str = r"(\d{4})-(\d{2})-(\d{2})T(\d{2})\:(\d{2})\:(\d{2})Z";

lazy_static! {
    static ref LOG_LINE_REGEX: Regex = Regex::new(&format!(
        r"^({})\s+(?:\[([^\]]+)\]\s+)?(.+)$",
        ISO8601_DATE_REGEX
    ))
    .unwrap();
    static ref BLOCK_CONNECTED_REGEX: Regex = Regex::new(&format!(
        r"Enqueuing BlockConnected: block hash=({}) block height=(\d+)",
        BLOCK_HASH_PATTERN
    ))
    .unwrap();
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

pub fn parse_log_event(line: &str) -> LogEvent {
    let (timestamp, category, message) = parse_common_log_data(line);

    let matchers: Vec<fn(&str) -> Option<Event>> = vec![BlockConnectedLog::parse_event];
    for matcher in &matchers {
        if let Some(event) = matcher(&message) {
            return LogEvent {
                log_timestamp: timestamp,
                category: category.into(),
                event: Some(event),
            };
        }
    }

    // if no matcher succeeds, return unknown
    LogEvent {
        log_timestamp: timestamp,
        category: category.into(),
        event: UnknownLogMessage::parse_event(&message),
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
    let timestamp = match OffsetDateTime::parse(timestamp_str, &Rfc3339) {
        Ok(dt) => dt.unix_timestamp() as u64,
        Err(_) => 0,
    };
    let log_type =
        match category.and_then(|cat| LogDebugCategory::from_str_name(&cat.to_uppercase())) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_matcher_unknown_log_message() {
        let log = "2025-10-02T02:31:14Z Verification progress: 50%";
        let log_event = parse_log_event(log);

        assert_eq!(log_event.log_timestamp, 1759372274);
        assert_eq!(log_event.category, LogDebugCategory::Unknown as i32);

        if let Some(Event::UnknownLogMessage(unknown_log)) = log_event.event {
            assert_eq!(unknown_log.raw_message, "Verification progress: 50%");
            return;
        }

        panic!("Expected UnknownLogMessage event");
    }

    #[test]
    fn test_log_matcher_unknown_log_message_with_category() {
        let log = "2025-10-02T02:31:21Z [net] Flushed 0 addresses to peers.dat  2ms";
        let log_event = parse_log_event(log);

        assert_eq!(log_event.log_timestamp, 1759372281);
        assert_eq!(log_event.category, LogDebugCategory::Net as i32);

        if let Some(Event::UnknownLogMessage(unknown_log)) = log_event.event {
            assert_eq!(
                unknown_log.raw_message,
                "Flushed 0 addresses to peers.dat  2ms"
            );
            return;
        }

        panic!("Expected UnknownLogMessage event");
    }

    #[test]
    fn test_log_matcher_block_connected() {
        let log = "2025-09-27T01:52:01Z [validation] Enqueuing BlockConnected: block hash=41109f31c8ca4d8683ab5571ba462292ddb8486dee6ecd2e62901accc7952f0b block height=437";
        let log_event = parse_log_event(log);

        assert_eq!(log_event.category, LogDebugCategory::Validation as i32);

        if let Some(Event::BlockConnectedLog(event)) = log_event.event {
            assert_eq!(
                event.block_hash,
                "41109f31c8ca4d8683ab5571ba462292ddb8486dee6ecd2e62901accc7952f0b"
            );
            assert_eq!(event.block_height, 437);
            return;
        }

        panic!("Expected BlockConnectedLog event");
    }
}
