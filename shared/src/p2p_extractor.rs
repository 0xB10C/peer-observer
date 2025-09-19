use std::fmt;

// structs are generated via the p2p-extractor.proto file
include!(concat!(env!("OUT_DIR"), "/p2p_extractor.rs"));

impl fmt::Display for PingDuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PingDuration({}ns)", self.duration)
    }
}

impl fmt::Display for p2p_extractor_event::Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            p2p_extractor_event::Event::PingDuration(duration) => write!(f, "{}", duration),
        }
    }
}
