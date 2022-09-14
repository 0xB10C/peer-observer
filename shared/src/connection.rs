use crate::bcc_types;
use crate::primitive::ConnType;
use std::fmt;

// structs are generated via the connection.proto file
include!(concat!(env!("OUT_DIR"), "/connection.rs"));

impl From<bcc_types::Connection> for Connection {
    fn from(conn: bcc_types::Connection) -> Self {
        let conn_type: ConnType = conn.conn_type().into();
        Connection {
            peer_id: conn.id,
            addr: conn.addr(),
            conn_type: conn_type as i32,
            network: Network::from(conn.network) as i32,
            net_group: conn.net_group,
        }
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Connection(id={}, addr={}, conn_type={}, network={}, net_group={})",
            self.peer_id, self.addr, self.conn_type, self.network, self.net_group
        )
    }
}

// Must be the same as defined by Bitoin Core.
// These are currently defined in https://github.com/bitcoin/bitcoin/blob/master/src/netaddress.h#L44

// Bitcoin Core comment:
// Addresses from these networks are not publicly routable on the global Internet.
const BITCOINCORE_NET_UNROUTABLE: u32 = 0;
const BITCOINCORE_NET_IPV4: u32 = 1;
const BITCOINCORE_NET_IPV6: u32 = 2;
const BITCOINCORE_NET_ONION: u32 = 3;
const BITCOINCORE_NET_I2P: u32 = 4;
const BITCOINCORE_NET_CJDNS: u32 = 5;
// Bitcoin Core comment:
// A set of addresses that represent the hash of a string or FQDN. We use
// them in AddrMan to keep track of which DNS seeds were used.
const BITCOINCORE_NET_INTERNAL: u32 = 6;

impl From<u32> for Network {
    fn from(network: u32) -> Self {
        match network {
            BITCOINCORE_NET_UNROUTABLE => Network::Unroutable,
            BITCOINCORE_NET_IPV4 => Network::Ipv4,
            BITCOINCORE_NET_IPV6 => Network::Ipv6,
            BITCOINCORE_NET_ONION => Network::Onion,
            BITCOINCORE_NET_I2P => Network::I2p,
            BITCOINCORE_NET_CJDNS => Network::Cjdns,
            BITCOINCORE_NET_INTERNAL => Network::Internal,
            _ => Network::Unknown,
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Network::Unroutable => write!(f, "Unroutable"),
            Network::Ipv4 => write!(f, "IPv4"),
            Network::Ipv6 => write!(f, "IPv6"),
            Network::Onion => write!(f, "Onion"),
            Network::I2p => write!(f, "I2P"),
            Network::Cjdns => write!(f, "CJDNS"),
            Network::Internal => write!(f, "Internal"),
            Network::Unknown => write!(f, "Unknown"),
        }
    }
}

impl fmt::Display for ClosedConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ClosedConnection(conn={}, time_established={})",
            self.conn, self.time_established
        )
    }
}

impl From<bcc_types::ClosedConnection> for ClosedConnection {
    fn from(cconn: bcc_types::ClosedConnection) -> Self {
        ClosedConnection {
            conn: cconn.connection.into(),
            time_established: cconn.time_established,
        }
    }
}

impl fmt::Display for EvictedConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "EvictedConnection(conn={}, time_established={})",
            self.conn, self.time_established
        )
    }
}

impl From<bcc_types::ClosedConnection> for EvictedConnection {
    fn from(econn: bcc_types::ClosedConnection) -> Self {
        EvictedConnection {
            conn: econn.connection.into(),
            time_established: econn.time_established,
        }
    }
}

impl fmt::Display for InboundConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "InboundConnection(conn={}, existing_connections={})",
            self.conn, self.existing_connections
        )
    }
}

impl From<bcc_types::InboundConnection> for InboundConnection {
    fn from(iconn: bcc_types::InboundConnection) -> Self {
        InboundConnection {
            conn: iconn.connection.into(),
            existing_connections: iconn.existing_connections,
        }
    }
}

impl fmt::Display for OutboundConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "OutboundConnection(conn={}, existing_connections={})",
            self.conn, self.existing_connections
        )
    }
}

impl From<bcc_types::OutboundConnection> for OutboundConnection {
    fn from(oconn: bcc_types::OutboundConnection) -> Self {
        OutboundConnection {
            conn: oconn.connection.into(),
            existing_connections: oconn.existing_connections,
        }
    }
}

impl fmt::Display for MisbehavingConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MisbehavingConnection(id={}, score_before={}, score_increase={}, message={}, threshold_exceeded={})", self.id, self.score_before, self.score_increase, self.xmessage, self.threshold_exceeded)
    }
}

impl From<bcc_types::MisbehavingConnection> for MisbehavingConnection {
    fn from(mconn: bcc_types::MisbehavingConnection) -> Self {
        MisbehavingConnection {
            id: mconn.id,
            score_before: mconn.score_before,
            score_increase: mconn.score_increase,
            xmessage: mconn.message(),
            threshold_exceeded: mconn.threshold_exceeded,
        }
    }
}

impl fmt::Display for connection_event::Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            connection_event::Event::Closed(closed) => write!(f, "{}", closed),
            connection_event::Event::Evicted(evicted) => write!(f, "{}", evicted),
            connection_event::Event::Inbound(inbound) => write!(f, "{}", inbound),
            connection_event::Event::Outbound(outbound) => write!(f, "{}", outbound),
            connection_event::Event::Misbehaving(misbehaving) => {
                write!(f, "{}", misbehaving)
            }
        }
    }
}
