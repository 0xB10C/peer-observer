pub mod p2p {
    include!(concat!(env!("OUT_DIR"), "/p2p.rs"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
