# peer-observer

Tool to monitor for P2P anomalies and attacks using well-behaving, passive
Bitcoin Core honeynodes (honeypot nodes).

## Components and their interaction

The peer-observer consists of multiple components. Primiarly an `extractor` that
extracts events from a Bitcoin Core node and multiple `tools` that process the
extracted data. The `extractor` and `tools` are connected with a [nanomsg]-based
PUB-SUB TCP connection. The exchanged messages are serialized protobuf structures.

The extractor is written in Rust and uses the Bitcoin Core tracepoints to extract
events like received and send P2P messages, open and closed P2P connections, mempool
changes, and more. This is implemented using the USDT capabilites of [libbpf-rs].
The Bitcoin P2P protocol messages are deserialized using [rust-bitcoin].

The tools are written in Python or Rust (or any other language that supports nanomsg
and protobuf). They subscribe to the nanomsg publisher. For example, the `logger` tool
simply prints out all messages that it receives, the `metrics` tool produces prometheus
metrics, and the `addr-connectivity` tool tests received addresses if they are reachable.
Python tools can make use of the `protobuf/python-types` to deserialize the Protobuf
messages while Rust tools can use the types from the `shared` Rust module. 

```
                                                ┌──────────────────────┐
                                      Nanomsg   │ Tools                │
                                      PUB-SUB   │                      │
                                         ┌──────┼──►logger             │
              Tracepoints                │      │                      │
┌───────────┐ via libbpf                 ├──────┼──►metrics            │
│  Bitcoin  │          ┌───────────┐     │      │                      │
│ Core Node ├──────────► extractor ├─────┼──────┼──►archiver           │
└───────────┘          └───────────┘     │      │                      │
                                         ├──────┼──►addr-connectivty   │
                                         │      │                      │
                                         └──────┼──►...                │
                                      protobuf  │                      │
                                      messages  └──────────────────────┘
```

[nanomsg]: https://github.com/nanomsg/nng.git
[libbpf-rs]: https://github.com/libbpf/libbpf-rs
[rust-bitcoin]: https://github.com/rust-bitcoin/rust-bitcoin


## Real-world usage

On public.peer.observer, I run a peer-observer instance with multiple
Bitcoin Core honeynodes. To avoid leaking the IP addresses of these honeynodes
(an P2P attacker would just not attack these), public access is limited.

Setting up a peer-observer instance is non-trivial as hooking into the Bitcoin
Core tracepoints requires elevated system privileges. Additionally, a few not-yet-merged
patches to Bitcoin Core are required at the moment. Documentation is sparse
or non-existent. Feel free to open an issue if you still want to set up an instance and
I'll do my best to add more documentation.
