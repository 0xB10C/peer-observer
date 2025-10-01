// TODO: Update

# `log` extractor

> publishes data fetched from log files

The peer-observer log-extractor reads lines from a bitcoind log pipe (named pipe / FIFO) and publishes the results as events into a NATS pub-sub queue.

## Example

For example, connect to a NATS server on `128.0.0.1:1234` and to a pipe being written to by bitcoind at `/tmp/bitcoind-pipe`:

```bash
$ cargo run --bin log-extractor -- --nats-address 128.0.0.1:1234 --bitcoind-pipe /tmp/bitcoind-pipe
```

Its important to run bitcoind using the pipe like this:

```bash
$ mkfifo /tmp/bitcoind-pipe
$ bitcoind > /tmp/bitcoind-pipe
```

Also an option, which is done in integration tests, it's to use `tail -f` to read from the actual log file:

```bash
$ mkfifo /tmp/bitcoind-pipe
$ tail -f ~/.bitcoin/debug.log > /tmp/bitcoind-pipe
```

## Usage

```
$ cargo run --bin log-extractor -- --help
The peer-observer log-extractor reads lines from a bitcoind log pipe (named pipe / FIFO) and publishes the results as events into a NATS pub-sub queue

Usage: log-extractor [OPTIONS] <--bitcoind-pipe <BITCOIND_PIPE>>

Options:
  -n, --nats-address <NATS_ADDRESS>    Address of the NATS server where the extractor will publish messages to [default: 127.0.0.1:4222]
  -b, --bitcoind-pipe <BITCOIND_PIPE>  Path to the bitcoind log pipe (named pipe / FIFO)
  -l, --log-level <LOG_LEVEL>          The log level the extractor should run with. Valid log levels are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html [default: DEBUG]
  -h, --help                           Print help
  -V, --version                        Print version
```
