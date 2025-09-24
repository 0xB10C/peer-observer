use std::fmt;

use crate::protobuf::log_extractor;

// structs are generated via the log_extractor.proto file
include!(concat!(env!("OUT_DIR"), "/log_extractor.rs"));

impl fmt::Display for UnknownLogMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "UnknownLogMessage({})", self.raw_message)
    }
}

impl fmt::Display for log_extractor::log_event::Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            log_extractor::log_event::Event::UnknownLogMessage(message) => write!(f, "{}", message),
        }
    }
}
