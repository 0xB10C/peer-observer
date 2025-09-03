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
# HELP peerobserver_conn_evicted_inbound Number of evicted inbund connections.
# TYPE peerobserver_conn_evicted_inbound counter
peerobserver_conn_evicted_inbound 13
# HELP peerobserver_conn_inbound Number of inbound connections.
# TYPE peerobserver_conn_inbound counter
peerobserver_conn_inbound 14
# HELP peerobserver_conn_inbound_current Number of currently open inbound connections.
# TYPE peerobserver_conn_inbound_current gauge
peerobserver_conn_inbound_current 115
# HELP peerobserver_conn_inbound_network Number of inbound connections by network.
# TYPE peerobserver_conn_inbound_network counter
peerobserver_conn_inbound_network{network="1"} 14
# HELP peerobserver_mempool_added Number of transactions added to the mempool.
# TYPE peerobserver_mempool_added counter
peerobserver_mempool_added 53
# HELP peerobserver_mempool_added_vbytes Number of vbytes added to the mempool.
# TYPE peerobserver_mempool_added_vbytes counter
peerobserver_mempool_added_vbytes 14807
...
```

## Grafana Dashboards

The `dashboards` directory of this tool contains a set of Grafana dashboards that can
be used together with the `metrics` tool. They are intened for a multi-node setup where
each node is labeled with a `host` name. This can be done by scraping the metrics in
Prometheus with a configuration similar to:

```yaml
scrape_configs:
- job_name: peer-observer-metrics
  scrape_interval: 15s
  static_configs:
  - labels:
      host: alice
    targets:
    - [ip-of-alice]:[metrics-tool-port]
  - labels:
      host: bob
    targets:
    - [ip-of-bob]:[metrics-tool-port]
...
```

Grafana Dashboard provisioning is documented [here](https://grafana.com/docs/grafana/latest/administration/provisioning/#dashboards).
Alternativly, the dashboards can be manually imported into Grafana too.

The dashboards are tagged with, for example, tags like: `connections`, `mempool`, `addr`, ...

Addtionally, dashboards intended for usage in a big monitoring playlist are tagged with `big-playlist`.
Recent versions of Grafana allow to add dashboards to playlists by tags.

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
