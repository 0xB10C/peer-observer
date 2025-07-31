#![cfg_attr(feature = "strict", deny(warnings))]

pub extern crate async_nats;
pub extern crate bitcoin;
pub extern crate clap;
pub extern crate corepc_client;
pub extern crate lazy_static;
pub extern crate log;
pub extern crate nats;
pub extern crate prometheus;
pub extern crate prost;
pub extern crate simple_logger;
pub extern crate tokio;

pub mod addrman;
pub mod ctypes;
pub mod event_msg;
pub mod mempool;
pub mod nats_subjects;
pub mod net_conn;
pub mod net_msg;
pub mod primitive;
pub mod rpc;
pub mod validation;

/// A minimal HTTP webserver (but not spec compliant) used to serve prometheus metrics via HTTP.
pub mod metricserver;
/// Utillity functions shared among peer-observer tools
pub mod util;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
