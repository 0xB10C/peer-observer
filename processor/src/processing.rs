use crate::types::P2PMessageMetadata;
use bitcoin::consensus::encode::Decodable;
use bitcoin::hashes::{sha256d, Hash};
use bitcoin::network::message::NetworkMessage;
use bitcoin::network::message::RawNetworkMessage;

pub fn process_p2p_message(meta: &P2PMessageMetadata, payload: &[u8]) {
    let mut raw_message: Vec<u8> = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    for (i, b) in meta.msg_type.iter().enumerate() {
        if *b == 0x00 {
            break;
        }
        raw_message[4 + i] = *b;
    }
    let payload_hash = sha256d::Hash::hash(payload);
    raw_message.append(&mut (meta.msg_size as u32).to_le_bytes().to_vec());
    raw_message.append(&mut payload_hash[..4].to_vec());
    raw_message.append(&mut payload.to_vec());

    let rnn = RawNetworkMessage::consensus_decode(raw_message.as_slice()).unwrap();

    match rnn.payload {
        NetworkMessage::Unknown { .. } => println!(
            "{} {:?}",
            if meta.msg_inbound {
                "inbound"
            } else {
                "outbound"
            },
            rnn
        ),
        _ => (),
    }
}
