use nng::options::protocol::pubsub::Subscribe;
use nng::options::Options;
use nng::{Protocol, Socket};

use prost::Message;
use shared::p2p;

const ADDRESS: &'static str = "tcp://127.0.0.1:8883";

fn main() {
    let sub = Socket::new(Protocol::Sub0).unwrap();
    sub.dial(ADDRESS).unwrap();

    let all_topics = vec![];
    sub.set_opt::<Subscribe>(all_topics).unwrap();

    loop {
        let msg = sub.recv().unwrap();
        let protobuf = p2p::Message::decode(msg.as_slice()).unwrap();

        //if protobuf.meta.as_ref().unwrap().command == "addr" {
        println!("{:?}", protobuf);
        //}
    }
}
