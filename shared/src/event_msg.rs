// structs are generated via the wrapper.proto file
include!(concat!(env!("OUT_DIR"), "/event_msg.rs"));

use crate::event_msg::event_msg::Event;
use std::time::SystemTime;

impl EventMsg {
    pub fn new(event: Event) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let timestamp = now.as_secs();
        let timestamp_subsec_micros = now.subsec_micros();
        EventMsg {
            timestamp,
            timestamp_subsec_micros,
            event: Some(event),
        };
    }
}
