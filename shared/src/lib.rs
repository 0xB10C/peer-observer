#![cfg_attr(feature = "strict", deny(warnings))]

pub extern crate bitcoin;

pub mod addrman;
pub mod ctypes;
pub mod net_conn;
pub mod net_msg;
pub mod primitive;
pub mod wrapper;

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
