# `log` extractor

> publishes parsed log events from debug.log files

The peer-observer log-extractor reads lines from a pipe to a Bitcoin node debug.log pipe (named pipe / FIFO) and publishes parsed lines as events into a NATS pub-sub queue.

## Example

For example, connect to a NATS server on `128.0.0.1:1234` and to a pipe being written to by bitcoind at `/tmp/bitcoind-pipe`:

```bash
$ cargo run --bin log-extractor -- --nats-address 128.0.0.1:1234 --bitcoind-pipe /tmp/bitcoind-pipe
```

The Bitcoin node stdout output can either be written to the pipe directly:

```bash
$ mkfifo /tmp/bitcoind-pipe
$ bitcoind -debug=validation > /tmp/bitcoind-pipe
```

Or `tail -f` can be used to read from a debug.log file:

```bash
$ mkfifo /tmp/bitcoind-pipe
$ tail -f ~/.bitcoin/debug.log > /tmp/bitcoind-pipe
```

Note that some log messages are only logged by the Bitcoin node when respective debug category is turned on.
This can be done with e.g. `-debug=validation`. See `bitcoind --help` for more categories.

## Usage

```
$ cargo run --bin log-extractor -- --help
The peer-observer log-extractor reads lines from a pipe to a Bitcoin node debug.log pipe (named pipe / FIFO) and publishes parsed lines as events into a NATS pub-sub queue

Usage: log-extractor [OPTIONS] <--bitcoind-pipe <BITCOIND_PIPE>>

Options:
  -n, --nats-address <NATS_ADDRESS>    Address of the NATS server where the extractor will publish messages to [default: 127.0.0.1:4222]
  -b, --bitcoind-pipe <BITCOIND_PIPE>  Path to the bitcoind log pipe (named pipe / FIFO)
  -l, --log-level <LOG_LEVEL>          The log level the extractor should run with. Valid log levels are "trace", "debug", "info", "warn", "error". See https://docs.rs/log/latest/log/enum.Level.html [default: DEBUG]
  -h, --help                           Print help
  -V, --version                        Print version
```
