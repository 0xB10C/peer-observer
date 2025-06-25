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
