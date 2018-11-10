use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use super::internal_txs::PeerTx;

pub type EventCreator = u64;
pub type EventHash = u64;

#[derive(Hash, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Event {
    pub id: u64,
    pub hash: EventHash,
    pub timestamp: u64,
    pub creator: EventCreator,
    pub self_parent: EventHash,
    pub other_parent: EventHash,
    pub round: u64,
    pub transactions: Vec<Vec<u8>>,
    pub internal_transactions: Vec<PeerTx>,
}

impl Event {
    pub fn new(
        id: u64,
        creator: EventCreator,
        self_parent: EventHash,
        other_parent: EventHash,
        transactions: Vec<Vec<u8>>,
        internal_transactions: Vec<PeerTx>,
    ) -> Event {
        let mut ev = Event {
            id,
            hash: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            creator,
            self_parent,
            other_parent,
            round: 0,
            transactions,
            internal_transactions,
        };

        ev.calc_hash();

        ev
    }

    pub fn calc_hash(&mut self) {
        if self.hash != 0 {
            return;
        }

        let mut hasher = DefaultHasher::new();

        self.hash(&mut hasher);

        self.hash = hasher.finish();
    }

    pub fn is_root(&self) -> bool {
        self.self_parent == 0
    }
}
