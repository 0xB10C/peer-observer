import os
import sys
import csv
import json

sys.path.insert(1, os.path.join(sys.path[0], '../../protobuf/p2pnetobserver_types_py'))
import wrapper_pb2

from nanomsg import Socket, SUB, SUB_SUBSCRIBE

def meta_to_row(meta):
    return [meta.peer_id, meta.addr, meta.conn_type, meta.inbound, meta.timestamp, meta.timestamp_subsec_micros, meta.command]

with Socket(SUB) as sub:
    sub.connect('tcp://127.0.0.1:8883')
    sub.set_string_option(SUB, SUB_SUBSCRIBE, b'')
    with open("pingpongs.csv", "a") as pingpongs, open("versions.csv", "a") as versions:
        wp = csv.writer(pingpongs)
        wv = csv.writer(versions)
        while True:
            wrapped = wrapper_pb2.Wrapper()
            wrapped.ParseFromString(sub.recv())
            # check if it's a P2P message
            if wrapped.HasField("msg"):
                msg = wrapped.msg
                meta = msg.meta
                if meta.inbound and meta.command == "ping":
                    row = meta_to_row(meta) + [msg.ping.value]
                    wp.writerow(row)
                elif not meta.inbound and meta.command == "pong":
                    row = meta_to_row(meta) + [msg.pong.value]
                    wp.writerow(row)
                elif meta.inbound and meta.command == "version":
                    row = meta_to_row(meta) + [msg.version.user_agent]
                    print(row)
                    wv.writerow(row)
