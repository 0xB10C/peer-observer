import asyncio
from nats.aio.client import Client as NATS
import json
import sys
import csv
sys.path.append("../../protobuf/python-types")

import event_msg_pb2

def event_to_row(event):
    return [
        event.timestamp,
        event.msg.meta.inbound,
        event.msg.getblocktxn.block_hash.hex(),
        json.dumps(list(event.msg.getblocktxn.tx_indexes))
    ]

async def main():
    nc = NATS()

    await nc.connect("nats://127.0.0.1:4222")

    async def message_handler(msg):
        subject = msg.subject
        data = msg.data

        event = event_msg_pb2.EventMsg()
        event.ParseFromString(data)

        p2pmsg = event.msg
        if p2pmsg.meta.command == "getblocktxn":
            row = event_to_row(event)
            print(row)
            with open("getblocktxns.csv", 'a') as csvfile:
                spamwriter = csv.writer(csvfile)
                spamwriter.writerow(row)
        
    await nc.subscribe("netmsg", cb=message_handler)
    print("Subscribed to 'netmsg' topic...")

    await asyncio.Future()

if __name__ == "__main__":
    asyncio.run(main())
