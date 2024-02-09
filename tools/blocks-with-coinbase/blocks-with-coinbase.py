import os
import sys
from nanomsg import Socket, SUB, SUB_SUBSCRIBE
from google.protobuf import text_format
from google.protobuf import json_format
sys.path.insert(1, os.path.join(sys.path[0], '../../protobuf/python-types'))
import wrapper_pb2

with Socket(SUB) as sub:
    sub.connect('tcp://127.0.0.1:8883')
    sub.set_string_option(SUB, SUB_SUBSCRIBE, b'')
    print("connected. waiting for block or cmpctblock...")
    while True:
        wrapped = wrapper_pb2.Wrapper()
        wrapped.ParseFromString(sub.recv())
        # check if it's a P2P message
        if wrapped.HasField("msg"):
            msg = wrapped.msg
            meta = msg.meta
            if meta.command == "block":
                print(f"{wrapped.timestamp}: block from {meta.peer_id}")
                msg_json = json_format.MessageToJson(msg)
                print(msg_json)
                print()
