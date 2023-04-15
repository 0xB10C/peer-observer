use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

// The generation code for the following includes!() can be found in build.rs.

// Includes an auto-generated function to check if IPs are on the gmax banlist
include!(concat!(env!("OUT_DIR"), "/gmaxbanlist.rs"));

// Includes an auto-generated function to check if IPs are on the monero banlist
include!(concat!(env!("OUT_DIR"), "/monerobanlist.rs"));

// Includes an auto-generated function to check if IPs are Tor exit node IPs
include!(concat!(env!("OUT_DIR"), "/torexitlist.rs"));

/// Split and return the IP from an ip:port combination.
pub fn ip_from_ipport(addr: String) -> String {
    match addr.rsplit_once(":") {
        Some((ip, _)) => ip.replace("[", "").replace("]", "").to_string(),
        None => addr,
    }
}

/// Returns the /24 subnet for IPv4 or the /64 subnet for IPv6 address.
/// If [ip] is not a valid IPv4 or IPv6 address, the original ip is returned.
/// This is the case for Tor and I2P addresses.
/// TODO: make sure this works for CJDNS IPs too.
pub fn subnet(ip: String) -> String {
    let cleaned_ip = ip.replace("[", "").replace("]", "");
    if let Ok(ip_addr) = cleaned_ip.parse() {
        match ip_addr {
            IpAddr::V4(a) => {
                let o = a.octets();
                return Ipv4Addr::new(o[0], o[1], o[2], 0).to_string();
            }
            IpAddr::V6(a) => {
                let s = a.segments();
                return Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0).to_string();
            }
        }
    }
    return ip;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_from_ipport() {
        assert_eq!(
            ip_from_ipport(String::from("127.0.0.1:8333")).as_str(),
            "127.0.0.1"
        );
        assert_eq!(ip_from_ipport(String::from("::1:8333")).as_str(), "::1");
        assert_eq!(
            ip_from_ipport(String::from("::ffff:a.b.c.d:8333")).as_str(),
            "::ffff:a.b.c.d"
        );
        assert_eq!(
            ip_from_ipport(String::from("[::ffff:a.b.c.d]:8333")).as_str(),
            "::ffff:a.b.c.d"
        );
    }

    #[test]
    fn test_subnet_24_or_64_or_ip() {
        assert_eq!(subnet(String::from("127.0.0.1")).as_str(), "127.0.0.0");
        assert_eq!(
            subnet(String::from("2604:d500:4:1::3:a2")).as_str(),
            "2604:d500:4:1::"
        );
        assert_eq!(
            subnet(String::from("[2604:d500:4:1::3:a2]")).as_str(),
            "2604:d500:4:1::"
        );
    }

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
