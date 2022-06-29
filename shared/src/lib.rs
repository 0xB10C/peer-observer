
pub extern crate bitcoin;

pub mod connection;
pub mod p2p;
pub mod primitive;
pub mod wrapper;

pub mod bcc_types;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
