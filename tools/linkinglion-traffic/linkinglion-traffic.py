import asyncio
from nats.aio.client import Client as NATS
import json
import sys
from datetime import datetime
from random import randrange
sys.path.append("../../protobuf/python-types")

import event_msg_pb2

def format_traffic(b):
    if b > 1_000_000_000:
        return "{:.2f}GB".format(b/1_000_000_000).rjust(10)
    elif b > 1_000_000:
        return "{:.2f}MB".format(b/1_000_000).rjust(10)
    elif b > 1_000:
        return "{:.2f}kB".format(b/1_000).rjust(10)
    else:
        return f"{b}b".rjust(10)


def print_traffic_detailed():
    print("---")
    print(datetime.utcnow())
    print(json.dumps(traffic, sort_keys=True, indent=4))

def print_traffic_short():
    down_o = traffic["other"]["download"]["total"]
    up_o = traffic["other"]["upload"]["total"]
    total_o = down_o + up_o
    print("other download:", format_traffic(down_o), "\tupload:", format_traffic(up_o), "\ttotal:", format_traffic(total_o))

    down_l = traffic["linking-lion"]["download"]["total"]
    up_l = traffic["linking-lion"]["upload"]["total"]
    total_l = down_l + up_l
    down_percentage = "down: {:.2f}%".format(down_l * 100 / (down_o + down_l))
    up_percentage = "up: {:.2f}%".format(up_l * 100 / (up_o + up_l))
    total_percentage = "t: {:.2f}%".format(total_l * 100 / (total_o + total_l))
    print("llion download:", format_traffic(down_l), "\tupload:", format_traffic(up_l), "\ttotal:", format_traffic(total_l), down_percentage, up_percentage, total_percentage)

traffic = {
    "linking-lion": {
        "download": {"total": 0},
        "upload": {"total": 0},
    },
    "other": {
        "download": {"total": 0},
        "upload": {"total": 0},
    },
}

def is_linking_lion(meta):
    for subnet in ["91.198.115", "162.218.65", "209.222.252", "2604:d500:"]:
        if subnet in meta.addr:
            return True
    return False

async def main():
    nc = NATS()
    await nc.connect("nats://127.0.0.1:4222")

    async def message_handler(msg):
        subject = msg.subject
        data = msg.data

        event = event_msg_pb2.EventMsg()
        event.ParseFromString(data)

        p2pmsg = event.msg
        dir = "download" if p2pmsg.meta.inbound else "upload"
        actor = "linking-lion" if is_linking_lion(p2pmsg.meta) else "other"
        command = p2pmsg.meta.command
        if command not in traffic[actor][dir]:
            traffic[actor][dir][command] = 0
        traffic[actor][dir][command] += p2pmsg.meta.size
        traffic[actor][dir]["total"] += p2pmsg.meta.size

        if randrange(100) == 0:
            print_traffic_short()
            print()
        if randrange(10000) == 0:
            print_traffic_detailed()


    await nc.subscribe("netmsg", cb=message_handler)
    print("Subscribed to 'netmsg' topic...")

    await asyncio.Future()

if __name__ == "__main__":
    asyncio.run(main())
