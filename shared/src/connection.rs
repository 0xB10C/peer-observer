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
            network: conn.network,
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

impl fmt::Display for ClosedConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ClosedConnection(conn={}, last_block_time={}, last_tx_time={}, last_ping_time={}, min_ping_time={}, relays_tx={})",
            self.conn, self.last_block_time, self.last_tx_time, self.last_ping_time, self.min_ping_time, self.relays_txs
        )
    }
}

impl From<bcc_types::ClosedConnection> for ClosedConnection {
    fn from(cconn: bcc_types::ClosedConnection) -> Self {
        ClosedConnection {
            conn: cconn.connection.into(),
            last_block_time: cconn.last_block_time,
            last_tx_time: cconn.last_tx_time,
            last_ping_time: cconn.last_ping_time,
            min_ping_time: cconn.min_ping_time,
            relays_txs: cconn.relays_txs,
        }
    }
}

impl fmt::Display for EvictedConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "EvictedConnection(conn={}, last_block_time={}, last_tx_time={}, last_ping_time={}, min_ping_time={}, relays_tx={})",
            self.conn, self.last_block_time, self.last_tx_time, self.last_ping_time, self.min_ping_time, self.relays_txs
        )
    }
}

impl From<bcc_types::ClosedConnection> for EvictedConnection {
    fn from(econn: bcc_types::ClosedConnection) -> Self {
        EvictedConnection {
            conn: econn.connection.into(),
            last_block_time: econn.last_block_time,
            last_tx_time: econn.last_tx_time,
            last_ping_time: econn.last_ping_time,
            min_ping_time: econn.min_ping_time,
            relays_txs: econn.relays_txs,
        }
    }
}

impl fmt::Display for InboundConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "InboundConnection(conn={}, services={}, inbound_onion={}, existing_connections={})",
            self.conn, self.services, self.inbound_onion, self.existing_connections,
        )
    }
}

impl From<bcc_types::InboundConnection> for InboundConnection {
    fn from(iconn: bcc_types::InboundConnection) -> Self {
        InboundConnection {
            conn: iconn.connection.into(),
            services: iconn.services,
            inbound_onion: iconn.inbound_onion,
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
