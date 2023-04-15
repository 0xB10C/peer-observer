// The generation code for the following includes!() can be found in build.rs.

// Includes an auto-generated function to check if IPs are on the gmax banlist
include!(concat!(env!("OUT_DIR"), "/gmaxbanlist.rs"));

// Includes an auto-generated function to check if IPs are on the monero banlist
include!(concat!(env!("OUT_DIR"), "/monerobanlist.rs"));

// Includes an auto-generated function to check if IPs are Tor exit node IPs
include!(concat!(env!("OUT_DIR"), "/torexitlist.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gmaxbanlistip() {
        assert!(!is_on_gmax_banlist("this is probably not a banned IP"));

        // The IP addresses defined in RFC5737 for use in documentation likely
        // won't be added to the banlist (?).
        //
        // https://www.rfc-editor.org/rfc/rfc5737
        assert!(!is_on_gmax_banlist("192.0.2.222")); // TEST-NET-1
        assert!(!is_on_gmax_banlist("198.51.100.111")); // TEST-NET-2
        assert!(!is_on_gmax_banlist("203.0.113.123")); // TEST-NET-3

        // These are actual IP addresses picked from the list. Some might be
        // removed for that list at some point and need to be changed here.
        assert!(is_on_gmax_banlist("101.206.168.254"));
        assert!(is_on_gmax_banlist("27.46.8.217"));
        assert!(is_on_gmax_banlist("93.201.99.189"));
    }

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
