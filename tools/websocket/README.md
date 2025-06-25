# `websocket` tool

> publishes events into a websocket as JSON

A peer-observer tool that sends out all events on a websocket. Can be used to
visualize the events in the browser. The `www/*.html` files implement a few
visualizations.

## Example

For example, connect to a NATS server on 128.0.0.1:1234 and start the websocket server on 127.0.0.1:4848:

```
$ cargo run --bin websocket -- --nats-address 127.0.0.1:1234 --websocket-address 127.0.0.1:4848

or

$ ./target/[release/debug]/websocket -n 127.0.0.1:1234 --websocket-address 127.0.0.1:4848
```

## Usage

```
$ cargo run --bin websocket -- --help
A peer-observer tool that sends out all events on a websocket

Usage: websocket [OPTIONS]

Options:
  -n, --nats-address <NATS_ADDRESS>
          The NATS server address the tool should connect and subscribe to [default: 127.0.0.1:4222]
  -w, --websocket-address <WEBSOCKET_ADDRESS>
          The websocket address the tool listens on [default: 127.0.0.1:47482]
  -l, --log-level <LOG_LEVEL>
          The log level the took should run with. Valid log levels are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html [default: DEBUG]
  -h, --help
          Print help
  -V, --version
          Print version
```
