// Tor v3 addresses are 62 chars + 6 chars for the port (':12345').
#define MAX_PEER_ADDR_LENGTH 62 + 6
#define MAX_PEER_CONN_TYPE_LENGTH 20
#define MAX_MISBEHAVING_MESSAGE_LENGTH 128

BPF_RINGBUF_OUTPUT(evicted_connections, 64);
BPF_RINGBUF_OUTPUT(closed_connections, 64);
BPF_RINGBUF_OUTPUT(inbound_connections, 64);
BPF_RINGBUF_OUTPUT(outbound_connections, 64);
BPF_RINGBUF_OUTPUT(misbehaving_connections, 64);

struct Connection
{
    u64     id;
    char    addr[MAX_PEER_ADDR_LENGTH];
    char    type[MAX_PEER_CONN_TYPE_LENGTH];
    u32     network;
    u64     net_group;
};

struct ClosedConnection
{
    struct Connection conn;
    u64     last_block_time;
    u64     last_tx_time;
    u64     last_ping_time;
    u64     min_ping_time;
    bool    relays_txs;
};

struct InboundConnection
{
    struct  Connection conn;
    u64     services;
    bool    inbound_onion;
    u64     existing_connections;
};

struct OutboundConnection
{
    struct  Connection conn;
    u64     existing_connections;
};

struct MisbehavingConnection
{
    u64     id;
    s32     score_before;
    s32     howmuch;
    char    message[MAX_MISBEHAVING_MESSAGE_LENGTH];
    bool    threshold_exceeded;
};

int trace_evicted_connection(struct pt_regs *ctx) {
    struct ClosedConnection evicted = {};

    bpf_usdt_readarg(1, ctx, &evicted.conn.id);
    bpf_usdt_readarg_p(2, ctx, &evicted.conn.addr, MAX_PEER_ADDR_LENGTH);
    bpf_usdt_readarg_p(3, ctx, &evicted.conn.type, MAX_PEER_CONN_TYPE_LENGTH);
    bpf_usdt_readarg(4, ctx, &evicted.conn.network);
    bpf_usdt_readarg(5, ctx, &evicted.conn.net_group);
    bpf_usdt_readarg(6, ctx, &evicted.last_block_time);
    bpf_usdt_readarg(7, ctx, &evicted.last_tx_time);
    bpf_usdt_readarg(8, ctx, &evicted.last_ping_time);
    bpf_usdt_readarg(9, ctx, &evicted.min_ping_time);
    bpf_usdt_readarg(10, ctx, &evicted.relays_txs);

    evicted_connections.ringbuf_output(&evicted, sizeof(evicted), 0);
    return 0;
};

int trace_closed_connection(struct pt_regs *ctx) {
    struct ClosedConnection closed = {};

    bpf_usdt_readarg(1, ctx, &closed.conn.id);
    bpf_usdt_readarg_p(2, ctx, &closed.conn.addr, MAX_PEER_ADDR_LENGTH);
    bpf_usdt_readarg_p(3, ctx, &closed.conn.type, MAX_PEER_CONN_TYPE_LENGTH);
    bpf_usdt_readarg(4, ctx, &closed.conn.network);
    bpf_usdt_readarg(5, ctx, &closed.conn.net_group);
    bpf_usdt_readarg(6, ctx, &closed.last_block_time);
    bpf_usdt_readarg(7, ctx, &closed.last_tx_time);
    bpf_usdt_readarg(8, ctx, &closed.last_ping_time);
    bpf_usdt_readarg(9, ctx, &closed.min_ping_time);
    bpf_usdt_readarg(10, ctx, &closed.relays_txs);

    closed_connections.ringbuf_output(&closed, sizeof(closed), 0);
    return 0;
};

int trace_inbound_connection(struct pt_regs *ctx) {
    struct InboundConnection inbound = {};

    bpf_usdt_readarg(1, ctx, &inbound.conn.id);
    bpf_usdt_readarg_p(2, ctx, &inbound.conn.addr, MAX_PEER_ADDR_LENGTH);
    bpf_usdt_readarg_p(3, ctx, &inbound.conn.type, MAX_PEER_CONN_TYPE_LENGTH);
    bpf_usdt_readarg(4, ctx, &inbound.conn.network);
    bpf_usdt_readarg(5, ctx, &inbound.conn.net_group);
    bpf_usdt_readarg(6, ctx, &inbound.services);
    bpf_usdt_readarg(7, ctx, &inbound.inbound_onion);
    bpf_usdt_readarg(8, ctx, &inbound.existing_connections);

    inbound_connections.ringbuf_output(&inbound, sizeof(inbound), 0);
    return 0;
};

int trace_outbound_connection(struct pt_regs *ctx) {
    struct OutboundConnection outbound = {};

    bpf_usdt_readarg(1, ctx, &outbound.conn.id);
    bpf_usdt_readarg_p(2, ctx, &outbound.conn.addr, MAX_PEER_ADDR_LENGTH);
    bpf_usdt_readarg_p(3, ctx, &outbound.conn.type, MAX_PEER_CONN_TYPE_LENGTH);
    bpf_usdt_readarg(4, ctx, &outbound.conn.network);
    bpf_usdt_readarg(5, ctx, &outbound.conn.net_group);
    bpf_usdt_readarg(6, ctx, &outbound.existing_connections);

    outbound_connections.ringbuf_output(&outbound, sizeof(outbound), 0);
    return 0;
};


int trace_misbehaving_connection(struct pt_regs *ctx) {
    struct MisbehavingConnection misbehaving = {};

    bpf_usdt_readarg(1, ctx, &misbehaving.id);
    bpf_usdt_readarg(2, ctx, &misbehaving.score_before);
    bpf_usdt_readarg(3, ctx, &misbehaving.howmuch);
    bpf_usdt_readarg_p(4, ctx, &misbehaving.message, 64);
    bpf_usdt_readarg(5, ctx, &misbehaving.threshold_exceeded);

    misbehaving_connections.ringbuf_output(&misbehaving, sizeof(misbehaving), 0);
    return 0;
};

