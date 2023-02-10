// includes an auto-generated function to match Tor exit node IPs
// the generation code can be found in build.rs
include!(concat!(env!("OUT_DIR"), "/torexitlist.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_tor_exit_node() {
        assert!(!is_tor_exit_node(
            "this is probably not an ip of a tor exit node"
        ));

        // The IP addresses defined in RFC5737 for use in documentation likely
        // won't be used as Tor exit nodes any time soon (?).
        //
        // https://www.rfc-editor.org/rfc/rfc5737
        assert!(!is_tor_exit_node("192.0.2.222")); // TEST-NET-1
        assert!(!is_tor_exit_node("198.51.100.111")); // TEST-NET-2
        assert!(!is_tor_exit_node("203.0.113.123")); // TEST-NET-3

        // These are actual IP addresses picked from the list. Some might be
        // removed for that list at some point and need to be changed here.
        assert!(is_tor_exit_node("185.220.100.253")); // tor-exit-2.zbau.f3netze.de
        assert!(is_tor_exit_node("162.247.72.199")); // jaffer.tor-exit.calyxinstitute.org
        assert!(is_tor_exit_node("185.220.102.248")); // tor-exit-relay-2.anonymizing-proxy.digitalcourage.de
    }
}
