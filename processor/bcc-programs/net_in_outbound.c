// Tor v3 addresses are 62 chars + 6 chars for the port (':12345').
// I2P addresses are 60 chars + 6 chars for the port (':12345').
#define MAX_PEER_ADDR_LENGTH 62 + 6
#define MAX_PEER_CONN_TYPE_LENGTH 20
#define MAX_MSG_TYPE_LENGTH 12

#define MIN(a,b) ({ __typeof__ (a) _a = (a); __typeof__ (b) _b = (b); _a < _b ? _a : _b; })


// We use 4 different max message sizes and try to use the smallest possible.
// We expect mostly SMALL and MEDIUM messages, a few large and very few HUGE
// messages. We don't want to allocate e.g. _HUGE_MSG_LENGTH_ bytes for a
// SMALL message.
#define MAX_SMALL_MSG_LENGTH 256
#define MAX_MEDIUM_MSG_LENGTH 4096
#define MAX_LARGE_MSG_LENGTH 65536
#define MAX_HUGE_MSG_LENGTH 4194304

struct Metadata
{
    u64     peer_id;
    char    peer_addr[MAX_PEER_ADDR_LENGTH];
    char    peer_conn_type[MAX_PEER_CONN_TYPE_LENGTH];
    char    msg_type[MAX_MSG_TYPE_LENGTH];
    bool    msg_inbound;
    u64     msg_size;
};

struct SmallP2PMessage
{
    struct Metadata   meta;
    u8                payload[MAX_SMALL_MSG_LENGTH];
};

struct MediumP2PMessage
{
    struct Metadata   meta;
    u8                payload[MAX_MEDIUM_MSG_LENGTH];
};

struct LargeP2PMessage
{
    struct Metadata   meta;
    u8                payload[MAX_LARGE_MSG_LENGTH];
};

struct HugeP2PMessage
{
    struct Metadata   meta;
    u8                payload[MAX_HUGE_MSG_LENGTH];
};


BPF_RINGBUF_OUTPUT(messages_small, 1024);
BPF_RINGBUF_OUTPUT(messages_medium, 1024);
BPF_RINGBUF_OUTPUT(messages_large, 1024);
BPF_RINGBUF_OUTPUT(messages_huge, 1024);


int trace_inbound_message_rb(struct pt_regs *ctx) {
    bool inbound = true;
    u64 msg_size;
    bpf_usdt_readarg(5, ctx, &msg_size);

    if (msg_size <= MAX_SMALL_MSG_LENGTH) {

      struct SmallP2PMessage *msg = messages_small.ringbuf_reserve(sizeof(struct SmallP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);
      bpf_usdt_readarg_p(6, ctx, &msg->payload, msg_size);
      messages_small.ringbuf_submit(msg, 0);

    } else if (msg_size <= MAX_MEDIUM_MSG_LENGTH) {

      struct MediumP2PMessage *msg = messages_medium.ringbuf_reserve(sizeof(struct MediumP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);
      bpf_usdt_readarg_p(6, ctx, &msg->payload, msg_size);
      messages_medium.ringbuf_submit(msg, 0);

    } else if (msg_size <= MAX_LARGE_MSG_LENGTH) {

      struct LargeP2PMessage *msg = messages_large.ringbuf_reserve(sizeof(struct LargeP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);
      bpf_usdt_readarg_p(6, ctx, &msg->payload, msg_size);
      messages_large.ringbuf_submit(msg, 0);

    } else  {

      struct HugeP2PMessage *msg = messages_huge.ringbuf_reserve(sizeof(struct HugeP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);

      // If the message is larger than MAX_HUGE_MSG_LENGTH, it's cut-off after
      // MAX_HUGE_MSG_LENGTH bytes.
      bpf_usdt_readarg_p(6, ctx, &msg->payload, MIN(msg_size, MAX_HUGE_MSG_LENGTH));

      messages_huge.ringbuf_submit(msg, 0);
    }

    return 0;
}

int trace_outbound_message(struct pt_regs *ctx) {

    bool inbound = false;
    u64 msg_size;
    bpf_usdt_readarg(5, ctx, &msg_size);

    if (msg_size <= MAX_SMALL_MSG_LENGTH) {

      struct SmallP2PMessage *msg = messages_small.ringbuf_reserve(sizeof(struct SmallP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);
      bpf_usdt_readarg_p(6, ctx, &msg->payload, msg_size);
      messages_small.ringbuf_submit(msg, 0);

    } else if (msg_size <= MAX_MEDIUM_MSG_LENGTH) {

      struct MediumP2PMessage *msg = messages_medium.ringbuf_reserve(sizeof(struct MediumP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);
      bpf_usdt_readarg_p(6, ctx, &msg->payload, msg_size);
      messages_medium.ringbuf_submit(msg, 0);

    } else if (msg_size <= MAX_LARGE_MSG_LENGTH) {

      struct LargeP2PMessage *msg = messages_large.ringbuf_reserve(sizeof(struct LargeP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);
      bpf_usdt_readarg_p(6, ctx, &msg->payload, msg_size);
      messages_large.ringbuf_submit(msg, 0);

    } else  {

      struct HugeP2PMessage *msg = messages_huge.ringbuf_reserve(sizeof(struct HugeP2PMessage));
      if (!msg) return 1;
      msg->meta.msg_inbound = inbound;
      bpf_usdt_readarg(1, ctx, &msg->meta.peer_id);
      bpf_usdt_readarg_p(2, ctx, &msg->meta.peer_addr, MAX_PEER_ADDR_LENGTH);
      bpf_usdt_readarg_p(3, ctx, &msg->meta.peer_conn_type, MAX_PEER_CONN_TYPE_LENGTH);
      bpf_usdt_readarg_p(4, ctx, &msg->meta.msg_type, MAX_MSG_TYPE_LENGTH);
      bpf_usdt_readarg(5, ctx, &msg->meta.msg_size);

      // If the message is larger than MAX_HUGE_MSG_LENGTH, it's cut-off after
      // MAX_HUGE_MSG_LENGTH bytes.
      bpf_usdt_readarg_p(6, ctx, &msg->payload, MIN(msg_size, MAX_HUGE_MSG_LENGTH));

      messages_huge.ringbuf_submit(msg, 0);
    }

    return 0;
};
