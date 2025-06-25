# `connectivity-check` tool

> connects to IP addresses received via `addr(v2)` messages and records the result

A peer-observer tool that checks the connectivity of received addr(v2) message entries
by connecting to the addresses and trying to do a `version` handshake. The results are
recorded in a CSV file. Additionally, the tool offers prometheus metrics about the
connection results.

## Example

For example, connect to a NATS server on 128.0.0.1:1234 and start the metrics server on 127.0.0.1:8220:

```
$ cargo run --bin connectivity-check -- --nats-address 127.0.0.1:1234 --metrics-address 127.0.0.1:8220

or

$ ./target/[release/debug]/connectivity-check -n 127.0.0.1:1234 --metrics-address 127.0.0.1:8220
```

This then outputs something similar to:

```
$ curl 127.0.0.1:8220/metrics
...
# HELP connectivitycheck_addr_successful_connection_tor_exit Number of successful connections to addresses that we received from a Tor exit node.
# TYPE connectivitycheck_addr_successful_connection_tor_exit counter
connectivitycheck_addr_successful_connection_tor_exit{addr_version="addrv2",network="IPv4"} 10
# HELP connectivitycheck_addr_tried Number of addresses tried.
# TYPE connectivitycheck_addr_tried counter
connectivitycheck_addr_tried{addr_version="addr",network="IPv4"} 205
connectivitycheck_addr_tried{addr_version="addr",network="IPv6"} 4
connectivitycheck_addr_tried{addr_version="addrv2",network="IPv4"} 264
connectivitycheck_addr_tried{addr_version="addrv2",network="IPv6"} 62
# HELP connectivitycheck_addr_tried_tor_exit Number of addresses tried that we received from a Tor exit node.
# TYPE connectivitycheck_addr_tried_tor_exit counter
connectivitycheck_addr_tried_tor_exit{addr_version="addrv2",network="IPv4"} 35
connectivitycheck_addr_tried_tor_exit{addr_version="addrv2",network="IPv6"} 8
# HELP connectivitycheck_addr_unsuccessful_connection Number of unsuccessful connections by source.
# TYPE connectivitycheck_addr_unsuccessful_connection counter
...
```

And the CSV file looks similar to:

```
$ head addr-connectivity.csv
result_timestamp,addr_address,addr_port,addr_services,addr_timestamp,addr_network_type,addr_version,source_address,source_id,source_tor_exit_node,result_success,result_cached,version,version_useragent,version_relay,version_version,version_services,version_start_height,version_nonce,version_timestamp,version_sender_address,version_sender_port
1750866237,IPv4(XXX.XXX.XXX.XX),8333,3080,1750866237,IPv4,addrv2,XXX.XXX.XXX.XXX,599090,false,true,false,true,/Satoshi:27.1.0/,true,70016,3080,902670,13730891488897458440,1750866235,0.0.0.0,0
1750866241,IPv6(2000:000:0000:b10c:0000::1),8333,3080,1750866240,IPv6,addrv2,XXX.XXX.XXX.X,false,false,false,false,,false,0,0,-1,0,0,,0
1750866241,IPv4(XXX.XXX.XXX.XX),8333,1097,1750866239,IPv4,addrv2,XXX.XXX.XXX.XX,1915565,false,false,false,false,,false,0,0,-1,0,0,,0
1750866243,IPv4(XXX.XXX.XXX.XX),8333,3145,1750866243,IPv4,addrv2,XXX.XXX.XXX.XX,1348139,false,false,false,false,,false,0,0,-1,0,0,,0
1750866243,IPv6(2000:0000:0000:0000::XXXX),8333,1032,1750866243,IPv6,addrv2,XXX.XXX.XXX.XX,1277361,false,false,false,false,,false,0,0,-1,0,0,,0
1750866244,IPv4(XXX.XXX.XXX.XX),8333,67111948,1750866242,IPv4,addrv2,XXX.XXX.XXX.XX,1879810,true,false,false,false,,false,0,0,-1,0,0,,0
1750866244,IPv4(XXX.XXX.XXX.XX),8334,1033,1750866244,IPv4,addrv2,XXX.XXX.XXX.XX,1717790,false,false,false,false,,false,0,0,-1,0,0,,0
1750866244,IPv4(XXX.XXX.XXX.XX),8333,1,1750866242,IPv4,addr,XXX.XXX.XXX.XX,2073554,false,false,false,false,,false,0,0,-1,0,0,,0
1750866245,IPv4(XXX.XXX.XXX.XX),8333,3081,1750866243,IPv4,addrv2,XXX.XXX.XXX.XX,1277361,false,false,false,false,,false,0,0,-1,0,0,,0
```

## Usage

```
$ cargo run --bin connectivity-check -- --help
A peer-observer tool that checks the connectivity of received addr(v2) message entries by connecting to the addresses and trying to do a `version` handshake. The results are recorded in a CSV file. Additionally, the tool offers prometheus metrics about the connection results

Usage: connectivity-check [OPTIONS]

Options:
  -n, --nats-address <NATS_ADDRESS>
          The NATS server address the tool should connect and subscribe to [default: 127.0.0.1:4222]
  -m, --metrics-address <METRICS_ADDRESS>
          The metrics server address the tool should listen on [default: 127.0.0.1:18282]
  -l, --log-level <LOG_LEVEL>
          The log level the tool should run with. Valid log levels are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html [default: DEBUG]
  -h, --help
          Print help
  -V, --version
          Print version
```
