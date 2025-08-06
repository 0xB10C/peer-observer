use crate::bitcoin::hashes::Hash;
use crate::ctypes;
use std::fmt;

// structs are generated via the validation.proto file
include!(concat!(env!("OUT_DIR"), "/validation.rs"));

impl From<ctypes::ValidationBlockConnected> for BlockConnected {
    fn from(connected: ctypes::ValidationBlockConnected) -> Self {
        BlockConnected {
            hash: connected.hash.to_vec(),
            height: connected.height,
            transactions: connected.transactions as i64,
            inputs: connected.inputs,
            sigops: connected.sigops as i64,
            connection_time: connected.connection_time,
        }
    }
}

impl fmt::Display for BlockConnected {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BlockConnected(hash={}, height={}, transactions={}, inputs={}, sigops={}, time={}ns)",
            bitcoin::BlockHash::from_slice(&self.hash).unwrap(),
            self.height,
            self.transactions,
            self.inputs,
            self.sigops,
            self.connection_time,
        )
    }
}

impl fmt::Display for validation_event::Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            validation_event::Event::BlockConnected(connected) => write!(f, "{}", connected),
        }
    }
}
