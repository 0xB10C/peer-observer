import os
import sys
import csv
import json

sys.path.insert(1, os.path.join(sys.path[0], '../../protobuf/python-types'))
import event_msg_pb2

from nanomsg import Socket, SUB, SUB_SUBSCRIBE

def data_to_row(meta, wrapped):
    return [meta.peer_id, meta.addr, meta.conn_type, meta.inbound, wrapped.timestamp, wrapped.timestamp_subsec_micros, meta.command]

with Socket(SUB) as sub:
    sub.connect('tcp://127.0.0.1:8883')
    sub.set_string_option(SUB, SUB_SUBSCRIBE, b'')
    with open("pingpongs.csv", "a") as pingpongs, open("versions.csv", "a") as versions:
        wp = csv.writer(pingpongs)
        wv = csv.writer(versions)
        while True:
            wrapped = event_msg_pb2.Wrapper()
            wrapped.ParseFromString(sub.recv())
            # check if it's a P2P message
            if wrapped.HasField("msg"):
                msg = wrapped.msg
                meta = msg.meta
                if meta.inbound and meta.command == "ping":
                    row = data_to_row(meta, wrapped) + [msg.ping.value]
                    print(row)
                    wp.writerow(row)
                elif not meta.inbound and meta.command == "pong":
                    row = data_to_row(meta, wrapped) + [msg.pong.value]
                    print(row)
                    wp.writerow(row)
                elif meta.inbound and meta.command == "version":
                    row = data_to_row(meta, wrapped) + [msg.version.user_agent]
                    print(row)
                    wv.writerow(row)
