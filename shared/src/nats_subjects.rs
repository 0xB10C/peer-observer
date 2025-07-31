use std::fmt;

const NATS_SUBJECT_ADDRMAN: &str = "addrman";
const NATS_SUBJECT_MEMPOOL: &str = "mempool";
const NATS_SUBJECT_NETMSG: &str = "netmsg";
const NATS_SUBJECT_NETCONN: &str = "netconn";
const NATS_SUBJECT_VALIDATION: &str = "validation";
const NATS_SUBJECT_RPC: &str = "rpc";

pub enum Subject {
    Addrman,
    Mempool,
    NetMsg,
    NetConn,
    Validation,
    Rpc,
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Subject::Addrman => write!(f, "{}", NATS_SUBJECT_ADDRMAN),
            Subject::Mempool => write!(f, "{}", NATS_SUBJECT_MEMPOOL),
            Subject::NetConn => write!(f, "{}", NATS_SUBJECT_NETCONN),
            Subject::NetMsg => write!(f, "{}", NATS_SUBJECT_NETMSG),
            Subject::Validation => write!(f, "{}", NATS_SUBJECT_VALIDATION),
            Subject::Rpc => write!(f, "{}", NATS_SUBJECT_RPC),
        }
    }
}
