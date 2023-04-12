// includes an auto-generated function to match IPs
// the generation code can be found in build.rs
include!(concat!(env!("OUT_DIR"), "/monerobanlist.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_monero_banlist_ip() {
        assert!(!is_on_monero_banlist("this is probably not a banned ip"));

        // The IP addresses defined in RFC5737 for use in documentation likely
        // aren't on the monero banlist (?).
        //
        // https://www.rfc-editor.org/rfc/rfc5737
        assert!(!is_on_monero_banlist("192.0.2.222")); // TEST-NET-1
        assert!(!is_on_monero_banlist("198.51.100.111")); // TEST-NET-2
        assert!(!is_on_monero_banlist("203.0.113.123")); // TEST-NET-3

        // These are actual IP addresses picked from the list. Some might be
        // removed for that list at some point and need to be changed here.
        assert!(is_on_monero_banlist("51.75.162.171"));
        assert!(is_on_monero_banlist("23.88.124.135"));
        assert!(is_on_monero_banlist("165.22.2.201"));
    }
}
