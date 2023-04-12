// includes an auto-generated function to match IPs
// the generation code can be found in build.rs
include!(concat!(env!("OUT_DIR"), "/gmaxbanlist.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_gmaxbanlistip() {
        assert!(!is_on_gmax_banlist(
            "this is probably not an ip of a tor exit node"
        ));

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
}
