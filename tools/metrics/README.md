# `metrics` tool

> produces prometheus metrics from events

## Example

For example, connect to a NATS server on 128.0.0.1:1234 and start the metrics HTTP server under 127.0.0.1:8001:

```
$ cargo run --bin metrics -- --nats-address 127.0.0.1:1234 --metrics-address 127.0.0.1:8001

or

$ ./target/[release/debug]/logger -n 127.0.0.1:1234 --metrics-address 127.0.0.1:8001
```

This then outputs something similar to:

```
$ curl 127.0.0.1:8001/metrics
# HELP peerobserver_conn_closed Number of closed connections.
# TYPE peerobserver_conn_closed counter
peerobserver_conn_closed 14
# HELP peerobserver_conn_closed_age_seconds Age (in seconds) of closed connections. The age of each closed connection is added to the metric.
# TYPE peerobserver_conn_closed_age_seconds counter
peerobserver_conn_closed_age_seconds 16
# HELP peerobserver_conn_closed_network Number of closed connections by network.
# TYPE peerobserver_conn_closed_network counter
peerobserver_conn_closed_network{network="1"} 14
# HELP peerobserver_conn_closed_subnet Number of closed connections by subnet.
# TYPE peerobserver_conn_closed_subnet counter
peerobserver_conn_closed_subnet{subnet="XX.XXX.XX.0"} 14
# HELP peerobserver_conn_evicted_inbound Number of evicted inbund connections.
# TYPE peerobserver_conn_evicted_inbound counter
peerobserver_conn_evicted_inbound 13
# HELP peerobserver_conn_evicted_inbound_withinfo Number of evicted inbound connections with information about their address and network.
# TYPE peerobserver_conn_evicted_inbound_withinfo counter
peerobserver_conn_evicted_inbound_withinfo{addr="XXX.X.XX.XXX",network="1"} 6
peerobserver_conn_evicted_inbound_withinfo{addr="XX.X.XXX.X",network="1"} 7
# HELP peerobserver_conn_inbound Number of inbound connections.
# TYPE peerobserver_conn_inbound counter
peerobserver_conn_inbound 14
# HELP peerobserver_conn_inbound_banlist_monero Number of inbound connections from IPs on the Monero banlist.
# TYPE peerobserver_conn_inbound_banlist_monero counter
peerobserver_conn_inbound_banlist_monero{addr="XX.XX.XX.XX"} 7
peerobserver_conn_inbound_banlist_monero{addr="XXX.XX.XX.XX"} 7
# HELP peerobserver_conn_inbound_current Number of currently open inbound connections.
# TYPE peerobserver_conn_inbound_current gauge
peerobserver_conn_inbound_current 115
# HELP peerobserver_conn_inbound_network Number of inbound connections by network.
# TYPE peerobserver_conn_inbound_network counter
peerobserver_conn_inbound_network{network="1"} 14
# HELP peerobserver_conn_inbound_subnet Number of inbound connections by subnet (where applicable).
# TYPE peerobserver_conn_inbound_subnet counter
peerobserver_conn_inbound_subnet{subnet="XX.XX.XXX.0"} 14
# HELP peerobserver_mempool_added Number of transactions added to the mempool.
# TYPE peerobserver_mempool_added counter
peerobserver_mempool_added 53
# HELP peerobserver_mempool_added_vbytes Number of vbytes added to the mempool.
# TYPE peerobserver_mempool_added_vbytes counter
peerobserver_mempool_added_vbytes 14807
...
```

The dashboards in ./dashboards can be used in Grafana. However, they might be a bit outdated.

## Usage

```
$ cargo run --bin logger -- --help
A peer-observer tool that produces Prometheus metrics for received events

Usage: metrics [OPTIONS]

Options:
  -n, --nats-address <NATS_ADDRESS>
          The NATS server address the tool should connect and subscribe to [default: 127.0.0.1:4222]
  -m, --metrics-address <METRICS_ADDRESS>
          The metrics server address the tool should listen on [default: 127.0.0.1:8282]
  -l, --log-level <LOG_LEVEL>
          The log level the tool should run with. Valid log levels are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html [default: DEBUG]
  -h, --help
          Print help
  -V, --version
          Print version
```
