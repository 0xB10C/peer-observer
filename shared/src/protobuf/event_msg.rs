// structs are generated via the wrapper.proto file
include!(concat!(env!("OUT_DIR"), "/event_msg.rs"));

use crate::protobuf::event_msg::event_msg::Event;
use log::trace;
use std::time::SystemTime;
use std::time::SystemTimeError;

impl EventMsg {
    pub fn new(event: Event) -> Result<EventMsg, SystemTimeError> {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
        let timestamp = now.as_secs();
        let timestamp_subsec_micros = now.subsec_micros();
        trace!("creating new EventMsg with event: {:?}", event);
        Ok(EventMsg {
            timestamp,
            timestamp_subsec_micros,
            event: Some(event),
        })
    }
}
