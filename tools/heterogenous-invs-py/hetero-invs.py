import os
import sys
import csv
import json

sys.path.insert(1, os.path.join(sys.path[0], '../../protobuf/p2pnetobserver_types_py'))
import wrapper_pb2

from nanomsg import Socket, SUB, SUB_SUBSCRIBE

def meta_to_row(meta):
    return [meta.peer_id, meta.addr, meta.conn_type, meta.inbound, meta.timestamp, meta.timestamp_subsec_micros, meta.command]

useragents = dict()

with Socket(SUB) as sub:
    sub.connect('tcp://127.0.0.1:8883')
    sub.set_string_option(SUB, SUB_SUBSCRIBE, b'')
    with open("hetero-invs2.csv", "a") as heteroinvs:
        w = csv.writer(heteroinvs)
        while True:
            wrapped = wrapper_pb2.Wrapper()
            wrapped.ParseFromString(sub.recv())
            # check if it's a P2P message
            if wrapped.HasField("msg"):
                msg = wrapped.msg
                meta = msg.meta
                if meta.command == "inv":
                    inv_items_per_type = dict()
                    for inv_item in list(msg.inv.items):
                        itype = inv_item.WhichOneof("item")
                        hex_hash = bytes(getattr(inv_item, itype)).hex()
                        if itype not in inv_items_per_type:
                            inv_items_per_type[itype] = list()
                        inv_items_per_type[itype].append(hex_hash)
                    # check if we have multiple inv types
                    if len(inv_items_per_type.keys()) > 1:
                        row = meta_to_row(meta) + [useragents[meta.peer_id] if meta.peer_id in useragents else "NONE"]  + [inv_items_per_type]
                        print(row)
                        w.writerow(row)
                elif meta.inbound and meta.command == "version":
                    useragents[meta.peer_id] = msg.version.user_agent
