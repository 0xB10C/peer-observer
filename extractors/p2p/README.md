# `p2p` extractor

> publishes data received via the P2P interface

A peer-observer extractor that receives an inbound connection from a Bitcoin node
and publishes selected P2P messages and measurements as events into a NATS pub-sub
queue.

While the p2p-extractor in theory supports multiple inbound connections from multiple
Bitcoin nodes, it can't (yet) differentiate between them.

## Example

For example, connect to a NATS server on 128.0.0.1:1234 and listen on 127.0.0.1:8555
for an inbound regtest Bitcoin node P2P connection. The Bitcoin node needs to be started with
`bitcoind --regtest --addnode=127.0.0.1:8555`.

```
$ cargo run --bin p2p-extractor -- --nats-address 128.0.0.1:1234 --p2p-network regtest --p2p-address 127.0.0.1:8555
```

## Usage

```
$ cargo run --bin p2p-extractor -- --help
The peer-observer p2p-extractor listens for a connection from a Bitcoin node and once connected, extracts events from exchanged P2P messages. It publishes the events into a NATS pub-sub queue

Usage: p2p-extractor [OPTIONS]

Options:
  -n, --nats-address <NATS_ADDRESS>    Address of the NATS server where the extractor will publish messages to [default: 127.0.0.1:4222]
  -l, --log-level <LOG_LEVEL>          The log level the extractor should run with. Valid log levels are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html [default: DEBUG]
      --p2p-address <P2P_ADDRESS>      Address of the P2P interface the P2P extractor will listen on. On the Bitcoin node side, the connection needs to be established with -addnode=<p2p_address> [default: 127.0.0.1:9333]
      --p2p-network <P2P_NETWORK>      Network (P2P) the Bitcoin node is on. This determines the network magic. The network magic of the p2p-extractor and the Bitcoin node must match [default: mainnet] [possible values: mainnet, testnet3, testnet4, signet, regtest]
      --ping-interval <PING_INTERVAL>  The p2p_extractor frequently pings the connected node to measure ping and backlog timings. This allows to configure the ping interval (in seconds) [default: 10]
      --disable-ping                   The p2p_extractor frequently pings the connected node to measure ping and backlog timings. This allows disabling the ping measurements
      --disable-addrv2                 The p2p_extractor publishes events for addresses the node annouces to us. This allows disabling the address annoucement events
      --disable-invs                   The p2p_extractor publishes events for invs the node annouces to us. This allows disabling the inv annoucement events
  -h, --help                           Print help
  -V, --version                        Print version
```
