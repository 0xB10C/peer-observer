use std::fmt;

// structs are generated via the rpc.proto file
include!(concat!(env!("OUT_DIR"), "/rpc.rs"));

// TODO: implement from json rpc type

impl fmt::Display for PeerInfos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let info_strs: Vec<String> = self.infos.iter().map(|i| i.to_string()).collect();
        write!(f, "PeerInfos([{}])", info_strs.join(", "))
    }
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PeerInfo(id={})",
            self.id,
            // TODO: could add some more interesting fields here
        )
    }
}

impl fmt::Display for rpc_event::Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            rpc_event::Event::PeerInfos(infos) => write!(f, "{}", infos),
        }
    }
}
