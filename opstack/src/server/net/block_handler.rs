use alloy::primitives::Address;
use libp2p::gossipsub::{IdentTopic, Message, MessageAcceptance, TopicHash};
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::SequencerCommitment;

pub struct BlockHandler {
    chain_id: u64,
    signer: Address,
    commitment_sender: Sender<SequencerCommitment>,
    blocks_v3_topic: IdentTopic,
}

impl BlockHandler {
    pub fn new(signer: Address, chain_id: u64) -> (Self, Receiver<SequencerCommitment>) {
        let (send, recv) = channel(256);
        let handler = Self {
            chain_id,
            signer,
            commitment_sender: send,
            blocks_v3_topic: IdentTopic::new(format!("/optimism/{}/2/blocks", chain_id)),
        };

        (handler, recv)
    }

    pub fn topics(&self) -> Vec<TopicHash> {
        vec![self.blocks_v3_topic.hash()]
    }

    pub fn handle(&self, msg: Message) -> MessageAcceptance {
        let Ok(commitment) = SequencerCommitment::new(&msg.data) else {
            return MessageAcceptance::Reject;
        };

        if commitment.verify(self.signer, self.chain_id).is_ok() {
            _ = self.commitment_sender.try_send(commitment);
            MessageAcceptance::Accept
        } else {
            MessageAcceptance::Reject
        }
    }
}
