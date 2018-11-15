use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::event::{Event, EventHash};
use super::peers::Peers;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamousType {
    True,
    False,
    Undefined,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundEvent {
    pub hash: u64,
    pub witness: bool,
    pub famous: FamousType,
    pub received: u64,
    pub timestamp: u64,
    pub votes: HashMap<EventHash, bool>,
}

impl RoundEvent {
    pub fn from_event(e: Event) -> RoundEvent {
        RoundEvent {
            hash: e.hash,
            witness: false,
            famous: FamousType::Undefined,
            received: 0,
            timestamp: 0,
            votes: HashMap::new(),
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Round {
    pub id: u64,
    // todo: remove this unecessary arc mutex
    pub events: HashMap<EventHash, Arc<RwLock<RoundEvent>>>,
    // todo: remove this unecessary arc mutex
    pub witnesses: HashMap<EventHash, Arc<RwLock<RoundEvent>>>,
    pub peers: Peers,
    pub decided: bool,
}

impl Round {
    pub fn new(id: u64) -> Round {
        Round {
            id,
            events: HashMap::new(),
            witnesses: HashMap::new(),
            peers: Peers::new(),
            decided: false,
        }
    }

    pub fn insert(&mut self, e: Event, is_witness: bool) {
        let round_event = Arc::new(RwLock::new(RoundEvent::from_event(e.clone())));

        if is_witness {
            (*round_event.write().unwrap()).witness = true;

            self.witnesses.insert(e.hash, round_event.clone());
        }

        self.events.insert(e.hash, round_event.clone());
    }
}
