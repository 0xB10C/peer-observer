# IP lists

## Tor exit node IPs

The Tor project provides a list of IP addresses of Tor exit nodes called
`torbulkexitlist`. A raw list can be dowloaded to update the vendored list
from:

```
https://check.torproject.org/torbulkexitlist
```

## Greg Maxwell's old Bitcoin banlist

A banlist previously maintained by Greg Maxwell. The downloaded list
needs to be preporcesed:

- replace /24 ranges with the actual IPs. The /10 range is ignored
- sort and deduplicate

```
https://web.archive.org/web/20200504061024/https://people.xiph.org/~greg/banlist.cli.txt
```

## Monero IP banlist

A banlist of IP addresses curated by the monero community. The downloaded list
needs to be preporcesed:

```
wget https://gui.xmr.pm/files/block.txt
python3 preprocess-monero-block-list.py

```

## LinkingLion banlist

A banlist of IP addresses for LinkingLion. Since this list is rather small and doesn't update frequently,
but includes a /64 IPv6 address, we don't keep a separate list file for it. The matching is implemented with
a `ip.starts_with()`, to avoid having to check all possibilities of the /64. 

```
162.218.65.0/24
209.222.252.0/24
91.198.115.0/24
2604:d500:4:1::/64
```
