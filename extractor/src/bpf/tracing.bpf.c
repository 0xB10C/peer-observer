// SPDX-License-Identifier: (LGPL-2.1 OR BSD-2-Clause)
/* Copyright (c) 2022 0xB10C */
#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>
#include <bpf/usdt.bpf.h>

#define RINGBUFFER(name, pages) struct {__uint(type, BPF_MAP_TYPE_RINGBUF); __uint(max_entries, PAGE_SIZE * pages); } name SEC(".maps");

#define MAX_PEER_ADDR_LENGTH 62 + 6
#define MAX_PEER_CONN_TYPE_LENGTH 20
#define MAX_MSG_TYPE_LENGTH 12

// We use 4 different max P2P message sizes and try to use the smallest possible.
// We expect mostly SMALL and MEDIUM messages, a few large and very few HUGE
// messages. We don't want to allocate e.g. HUGE_MSG_LENGTH bytes for a
// SMALL message.
#define MAX_SMALL_MSG_LENGTH 256
#define MAX_MEDIUM_MSG_LENGTH 4096
#define MAX_LARGE_MSG_LENGTH 65536
#define MAX_HUGE_MSG_LENGTH 4194304

#define PAGE_SIZE 4096
#define NET_MSG_PAGES 128

// NET MESSAGES

RINGBUFFER(net_msg_small, NET_MSG_PAGES)
RINGBUFFER(net_msg_medium, NET_MSG_PAGES)
RINGBUFFER(net_msg_large, NET_MSG_PAGES)
RINGBUFFER(net_msg_huge, NET_MSG_PAGES)

struct Metadata {
    u64     id;
    char    addr[MAX_PEER_ADDR_LENGTH];
    char    conn_type[MAX_PEER_CONN_TYPE_LENGTH];
    char    msg_type[MAX_MSG_TYPE_LENGTH];
    bool    msg_inbound;
    u64     msg_size;
};

struct SmallP2PMessage {
    struct Metadata   meta;
    u8                payload[MAX_SMALL_MSG_LENGTH];
};

struct MediumP2PMessage {
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

// Helper function to set some of the tracepoint arguments to Metadata.
void set_meta_data1(struct Metadata *meta, u64 id, bool inbound, u64 msg_size) {
  meta->id = id;
  meta->msg_inbound = inbound;
  meta->msg_size = msg_size;
}

// Helper function to set some of the tracepoint arguments to Metadata.
void set_meta_data2(struct Metadata *meta, void *addr, void* conn_type, void* msg_type) {
  bpf_probe_read_str(&meta->addr, sizeof(meta->addr), addr);
  bpf_probe_read_str(&meta->conn_type, sizeof(meta->conn_type), conn_type);
  bpf_probe_read_str(&meta->msg_type, sizeof(meta->msg_type), msg_type);
}

SEC("usdt")
int BPF_USDT(handle_net_msg_inbound, u64 id, void *addr, void *conn_type, void *msg_type, u64 msg_size, void *msg_payload)
{
  bool IS_INBOUND = true;
  if (msg_size <= MAX_SMALL_MSG_LENGTH) {
    struct SmallP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_small, sizeof(struct SmallP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  } else if (msg_size <= MAX_MEDIUM_MSG_LENGTH) {
    struct MediumP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_medium, sizeof(struct MediumP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read_str(&msg->meta.msg_type, sizeof(msg->meta.msg_type), msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  } else if (msg_size <= MAX_LARGE_MSG_LENGTH) {
    struct LargeP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_large, sizeof(struct LargeP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  } else if (msg_size <= MAX_HUGE_MSG_LENGTH) {
    struct HugeP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_huge, sizeof(struct HugeP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  }

  return -1;
}

SEC("usdt")
int BPF_USDT(handle_net_msg_outbound, u64 id, void *addr, void *conn_type, void *msg_type, u64 msg_size, void *msg_payload)
{
  bool IS_INBOUND = false;
  if (msg_size <= MAX_SMALL_MSG_LENGTH) {
    struct SmallP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_small, sizeof(struct SmallP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  } else if (msg_size <= MAX_MEDIUM_MSG_LENGTH) {
    struct MediumP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_medium, sizeof(struct MediumP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read_str(&msg->meta.msg_type, sizeof(msg->meta.msg_type), msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  } else if (msg_size <= MAX_LARGE_MSG_LENGTH) {
    struct LargeP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_large, sizeof(struct LargeP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  } else if (msg_size <= MAX_HUGE_MSG_LENGTH) {
    struct HugeP2PMessage *msg = bpf_ringbuf_reserve(&net_msg_huge, sizeof(struct HugeP2PMessage), 0);
    if (msg) {
      set_meta_data1(&msg->meta, id, IS_INBOUND, msg_size);
      set_meta_data2(&msg->meta, addr, conn_type, msg_type);
      bpf_probe_read(&msg->payload, sizeof(msg->payload), msg_payload);
      bpf_ringbuf_submit(msg, 0);
      return 0;
    }
  }

  return -1;
}

// NET CONNECTIONS

#define MAX_MISBEHAVING_MESSAGE_LENGTH 128

#define NET_CONN_PAGES 64

RINGBUFFER(net_conn_inbound, NET_CONN_PAGES)
RINGBUFFER(net_conn_outbound, NET_CONN_PAGES)
RINGBUFFER(net_conn_closed, NET_CONN_PAGES)
RINGBUFFER(net_conn_evicted, NET_CONN_PAGES)
RINGBUFFER(net_conn_misbehaving, NET_CONN_PAGES)

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
    u64    time_established;
};

struct InboundConnection
{
    struct  Connection conn;
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

// Helper function to set some of the tracepoint arguments to Connection.
void set_conn_data1(struct Connection *conn, u64 id, u64 network, u64 net_group) {
  conn->id = id;
  conn->network = network;
  conn->net_group = net_group;
}

// Helper function to set some of the tracepoint arguments to Connection.
void set_conn_data2(struct Connection *conn, void *addr, void *type) {
  bpf_probe_read_str(&conn->addr, sizeof(conn->addr), addr);
  bpf_probe_read_str(&conn->type, sizeof(conn->type), type);
}

SEC("usdt")
int BPF_USDT(handle_net_conn_inbound, u64 id, void *addr, void *type, u64 network, u64 net_group, u64 existing_connections) {
    struct InboundConnection inbound = {};
    set_conn_data1(&inbound.conn, id, network, net_group);
    set_conn_data2(&inbound.conn, addr, type);
    inbound.existing_connections = existing_connections;
    return bpf_ringbuf_output(&net_conn_inbound, &inbound, sizeof(inbound), 0);
};

SEC("usdt")
int BPF_USDT(handle_net_conn_outbound, u64 id, void *addr, void *type, u64 network, u64 net_group, u64 existing_connections) {
    struct OutboundConnection outbound = {};
    set_conn_data1(&outbound.conn, id, network, net_group);
    set_conn_data2(&outbound.conn, addr, type);
    outbound.existing_connections = existing_connections;
    return bpf_ringbuf_output(&net_conn_outbound, &outbound, sizeof(outbound), 0);
};

SEC("usdt")
int BPF_USDT(handle_net_conn_closed, u64 id, void *addr, void *type, u64 network, u64 net_group, u64 time_established) {
    struct ClosedConnection closed = {};
    set_conn_data1(&closed.conn, id, network, net_group);
    set_conn_data2(&closed.conn, addr, type);
    closed.time_established = time_established;
    return bpf_ringbuf_output(&net_conn_closed, &closed, sizeof(closed), 0);
};

SEC("usdt")
int BPF_USDT(handle_net_conn_evicted, u64 id, void *addr, void *type, u64 network, u64 net_group, u64 time_established) {
    struct ClosedConnection evicted = {};
    set_conn_data1(&evicted.conn, id, network, net_group);
    set_conn_data2(&evicted.conn, addr, type);
    evicted.time_established = time_established;
    return bpf_ringbuf_output(&net_conn_evicted, &evicted, sizeof(evicted), 0);
};

SEC("usdt")
int BPF_USDT(handle_net_conn_misbehaving, u64 id, s32 score_before, s32 howmuch, void *message, bool threshold_exceeded) {
    struct MisbehavingConnection misbehaving = {};
    misbehaving.id = id;
    misbehaving.score_before = score_before;
    misbehaving.howmuch = howmuch;
    bpf_probe_read_str(&misbehaving.message, sizeof(misbehaving.message), message);
    misbehaving.threshold_exceeded = threshold_exceeded;
    return bpf_ringbuf_output(&net_conn_misbehaving, &misbehaving, sizeof(misbehaving), 0);
};




char LICENSE[] SEC("license") = "Dual BSD/GPL";
