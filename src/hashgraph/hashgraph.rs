use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use super::event::{Event, EventCreator, EventHash};
use super::events::{Events, EventsDiff, Frame};
use super::internal_txs::{PeerTx, PeerTxType};
use super::peer::Peer;
use super::peers::Peers;
use super::round::{FamousType, Round, RoundEvent};
use super::trace_time;

#[derive(Debug, Clone)]
pub struct Hashgraph {
    // pub peers: Arc<RwLock<Peers>>,
    pub events: Events,
    // todo: remove this unecessary arc mutex
    pub rounds: BTreeMap<u64, Round>, // round_id -> round
    pub tx_out: Arc<Mutex<Sender<Vec<u8>>>>,
    pub transactions: Vec<Vec<u8>>,
    pub internal_transactions: Vec<PeerTx>,

    ancestor_cache: HashMap<(EventHash, EventHash), bool>,
    first_decendant_cache: HashMap<(EventHash, EventHash), EventHash>,
    self_ancestor_cache: HashMap<(EventHash, EventHash), bool>,
    ss_cache: HashMap<(EventHash, EventHash), bool>,
    ss_path_cache: HashMap<(EventHash, EventHash), (bool, Vec<EventCreator>)>,
}

impl Default for Hashgraph {
    fn default() -> Hashgraph {
        let (tx_out, _) = channel();

        Hashgraph::new(
            // Arc::new(RwLock::new(Peers::new())),
            Arc::new(Mutex::new(tx_out)),
        )
    }
}

impl Hashgraph {
    pub fn new(tx_out: Arc<Mutex<Sender<Vec<u8>>>>) -> Hashgraph {
        // let mut first_round = Round::new(1);
        // let mut rounds = ;

        // first_round.peers = peers.read().unwrap().clone();

        // // TODO, dont start at 1
        // rounds.insert(1, Arc::new(RwLock::new(first_round))); // rounds start at 1

        Hashgraph {
            // peers,
            events: Events::new(),
            rounds: BTreeMap::new(),
            transactions: vec![],
            internal_transactions: vec![],
            tx_out,
            ancestor_cache: HashMap::new(),
            first_decendant_cache: HashMap::new(),
            self_ancestor_cache: HashMap::new(),
            ss_cache: HashMap::new(),
            ss_path_cache: HashMap::new(),
        }
    }

    // used by first node to setup the first round
    pub fn bootstrap(&mut self, peers: Peers) {
        let mut first_round = Round::new(1);

        first_round.peers = peers.clone();

        self.rounds.insert(1, first_round); // rounds start at 1
    }

    pub fn add_self_event(&mut self, tx: Vec<u8>, _peer_txs: Vec<PeerTx>) -> bool {
        let peer_txs = _peer_txs.clone();

        let self_id = self.get_last_decided_peers().self_id;

        let last_own_event = self.events.get_last_event_of(self_id);

        if last_own_event.is_none() {
            error!("Add self event: no events from self");

            return false;
        }

        let last_own_event = last_own_event.unwrap();

        self.insert_event(Event::new(
            last_own_event.id + 1,
            self_id,
            last_own_event.hash,
            0,
            vec![tx],
            peer_txs,
        ))
    }

    pub fn insert_event(&mut self, event: Event) -> bool {
        trace_time!("Insert Event");

        let mut event = event.clone();

        event.round = 0;

        if !self.events.check_event(&event) {
            return false;
        }

        // let round = self.get_parent_round(event.clone());

        event.round = self.get_round_id(event.clone());

        if self
            .get_decided_peers(&event)
            .get_by_id(event.creator)
            .is_none()
        {
            debug!("Error: Insert event: Peer not in the round: {:?}", event);

            return false;
        }

        self.add_to_round(event.clone());

        self.events.insert_event(event.clone());

        self.process_fame(event.clone());

        true
    }

    pub fn get_last_decided_peers(&self) -> Peers {
        // error!("RETURNING LAST PEERS !!!!!!!");
        let round = self.rounds.values().last().unwrap().clone();

        round.peers.clone()
    }

    // pub fn get_first_decided_peers(&self) -> Peers {
    //     error!("RETURNING FIRST PEERS !!!!!!!");
    //     let round = self.rounds.read().unwrap().values().next().unwrap().clone();

    //     round.peers.clone()
    // }

    pub fn get_decided_peers(&self, event: &Event) -> Peers {
        // error!("DECIDED PEER {:?}", event.round);
        if event.round == 0 {
            return self.get_last_decided_peers();
        }

        match self.rounds.get(&event.round) {
            Some(round) => round.peers.clone(),
            // None => match self.rounds.get(&(event.round - 1)) {
            //     Some(round) => round.peers.clone(),
            None => self.get_last_decided_peers(),
            // },
        }
    }

    pub fn merge_events(
        &mut self,
        self_id: u64,
        peer_id: u64,
        other_events: EventsDiff,
    ) -> Result<EventsDiff, String> {
        trace_time!("Merge Event");

        let mut merged = 0;

        for (hash, events) in other_events.diff {
            for event in events.values() {
                self.insert_event(event.clone());

                merged += 1;
            }
        }

        // todo: post checks to validate other parents

        if other_events.has_more {
            warn!("Has more");

            return Err("Has more".to_string());
        }

        let last_own_event = self.events.get_last_event_of(self_id);

        // syncing
        if last_own_event.is_none() {
            warn!("No own event {}", self_id);

            return Err(format!("No own event {}", self_id));
        }

        let last_own_event = last_own_event.unwrap();

        let last_other_event = self.events.get_last_event_of(peer_id);

        if last_other_event.is_none() {
            error!("Merge Events: Unknown peer");

            return Err("Merge Events: Unknown peer".to_string());
        }

        let last_other_event = last_other_event.unwrap();

        self.insert_event(Event::new(
            last_own_event.id + 1,
            self_id,
            last_own_event.hash,
            last_other_event.hash,
            vec![],
            vec![],
        ));

        let mut events_diff = self.events.events_diff(other_events.known, 0);

        events_diff.sender_id = self_id;

        trace!("Merged Event count: {}", merged);

        Ok(events_diff)
    }

    pub fn is_ancestor(&mut self, possible_ancestor: Event, e: Event) -> bool {
        let hash = (possible_ancestor.hash, e.hash);

        if let Some(res) = self.ancestor_cache.get(&hash) {
            return res.clone();
        }

        let res = self._is_ancestor(possible_ancestor, e);

        self.ancestor_cache.insert(hash, res);

        res
    }

    pub fn _is_ancestor(&mut self, possible_ancestor: Event, e: Event) -> bool {
        if possible_ancestor.hash == e.hash {
            return true;
        }

        let self_parent = self.events.get_event(&e.self_parent);
        let other_parent = self.events.get_event(&e.other_parent);

        if self_parent.is_none() {
            return false;
        }

        let self_parent = self_parent.unwrap();

        if self_parent.hash == possible_ancestor.hash {
            return true;
        }

        if other_parent.is_some() {
            let other_parent = other_parent.unwrap();

            if other_parent.hash == possible_ancestor.hash {
                return true;
            }

            if self.is_ancestor(possible_ancestor.clone(), other_parent) {
                return true;
            }
        }

        if self.is_ancestor(possible_ancestor.clone(), self_parent) {
            return true;
        }

        false
    }

    pub fn is_self_ancestor(&mut self, possible_ancestor: Event, e: Event) -> bool {
        let hash = (possible_ancestor.hash, e.hash);

        if let Some(res) = self.self_ancestor_cache.get(&hash) {
            return res.clone();
        }

        let res = self._is_self_ancestor(possible_ancestor, e);

        self.self_ancestor_cache.insert(hash, res);

        res
    }

    pub fn _is_self_ancestor(&mut self, possible_ancestor: Event, e: Event) -> bool {
        let self_parent = self.events.get_event(&e.self_parent);

        if self_parent.is_none() {
            return false;
        }

        let self_parent = self_parent.unwrap();

        if self_parent.hash == possible_ancestor.hash {
            return true;
        }

        if self.is_self_ancestor(possible_ancestor.clone(), self_parent) {
            return true;
        }

        false
    }

    pub fn see(&mut self, e: Event, possible_see: Event) -> bool {
        self.is_ancestor(possible_see, e)
    }

    pub fn strongly_see(&mut self, e: Event, possible_see: Event) -> bool {
        let hash = (e.hash, possible_see.hash);

        if let Some(res) = self.ss_cache.get(&hash) {
            return res.clone();
        }

        let res = self._strongly_see(e, possible_see);

        self.ss_cache.insert(hash, res);

        res
    }

    pub fn _strongly_see(&mut self, e: Event, possible_see: Event) -> bool {
        let super_majority = self.get_decided_peers(&possible_see).super_majority;

        let res = self.strongly_see_with_path(e, possible_see);

        res.0 == true && res.1.len() >= super_majority as usize
    }

    pub fn strongly_see_with_path(
        &mut self,
        e: Event,
        possible_see: Event,
    ) -> (bool, Vec<EventCreator>) {
        let hash = (e.hash, possible_see.hash);

        if let Some(res) = self.ss_path_cache.get(&hash) {
            return res.clone();
        }

        let res = self._strongly_see_with_path(e, possible_see);

        self.ss_path_cache.insert(hash, res.clone());

        res
    }

    fn _strongly_see_with_path(
        &mut self,
        e: Event,
        possible_see: Event,
    ) -> (bool, Vec<EventCreator>) {
        let self_parent = self.events.get_event(&e.self_parent);
        let other_parent = self.events.get_event(&e.other_parent);

        if let Some(other_parent) = other_parent {
            if other_parent.hash == possible_see.hash {
                return (true, vec![e.creator, other_parent.creator]);
            }

            if let (true, mut v) =
                self.strongly_see_with_path(other_parent.clone(), possible_see.clone())
            {
                v.push(e.creator);
                v.push(other_parent.creator);
                v.sort();
                v.dedup();

                return (true, v);
            }
        }

        if let Some(self_parent) = self_parent {
            if self_parent.hash == possible_see.hash {
                return (true, vec![self_parent.creator]);
            }

            if let (true, mut v) =
                self.strongly_see_with_path(self_parent.clone(), possible_see.clone())
            {
                v.push(self_parent.creator);
                v.sort();
                v.dedup();

                return (true, v);
            }
        }

        (false, vec![])
    }

    pub fn is_witness(&mut self, e: Event) -> bool {
        if e.is_root() {
            return true;
        }

        let last_round = self.get_parent_round(e.clone());

        let mut ss_count = 0;

        for witness_hash in last_round.witnesses.iter() {
            let witness_round_event = last_round.events.get(&witness_hash).unwrap();
            let got_witness = self.events.get_event(&witness_round_event.hash).unwrap();

            if self.strongly_see(e.clone(), got_witness) {
                ss_count += 1;
            }
        }

        // error!(
        //     "WITNESS {} {}",
        //     ss_count,
        //     self.get_decided_peers(&e).super_majority
        // );

        ss_count >= last_round.peers.super_majority
        // ss_count >= self.get_decided_peers(&e).super_majority
    }

    pub fn get_round_id(&mut self, e: Event) -> u64 {
        let e = e.clone();

        let is_witness = self.is_witness(e.clone());

        let last_round = self.get_parent_round(e.clone());
        let mut last_round_id = last_round.id;

        if !e.is_root() && is_witness {
            last_round_id += 1;
        }

        last_round_id
    }

    pub fn add_to_round(&mut self, e: Event) -> bool {
        if e.round == 0 {
            return false;
        }

        let e = e.clone();
        let last_round = self.get_parent_round(e.clone());
        let is_witness = self.is_witness(e.clone());

        self.rounds
            .entry(e.round)
            .and_modify(|round| {
                round.insert(e.clone(), is_witness);
            })
            .or_insert_with(|| {
                let mut round = Round::new(e.round);

                round.peers = last_round.peers.clone();

                round.insert(e.clone(), is_witness);

                round
            });

        true
    }

    pub fn get_last_populated_round(&self, event: &Event) -> Round {
        for round in self.rounds.values().rev() {
            if round.clone().peers.get_by_id(event.creator).is_some() {
                return round.clone();
            }
        }
        // for round in self.rounds.values().rev() {
        //     if round.events.len() > 0 {
        //         return round.clone();
        //     }
        // }

        self.rounds.values().last().unwrap().clone()
    }

    pub fn get_parent_round_id(&self, e: Event) -> u64 {
        let round = self.get_parent_round(e);

        let res = round;

        res.id
    }

    pub fn get_parent_round(&self, e: Event) -> Round {
        let self_parent = self.events.get_event(&e.self_parent);

        // let mut round = self.rounds.iter().last().unwrap().0.clone();
        let mut round = self.get_last_populated_round(&e).id;

        if self_parent.is_some() {
            round = self_parent.unwrap().round
        }

        self.rounds.get(&round).unwrap().clone()
    }

    pub fn process_fame(&mut self, e: Event) {
        // warn!("PROCESS FAME {} {}", e.round, self.is_witness(e.clone()));
        let first_round_nb = self.rounds.keys().next().unwrap().clone();

        if e.round == first_round_nb || !self.is_witness(e.clone()) {
            return;
        }

        trace_time!("Process Fame");

        let prev_round = self.rounds.get(&(e.round - 1)).unwrap().clone();
        // let mut round = self.rounds.get_mut(&e.round).unwrap();
        let mut round_event = self
            .rounds
            .get(&e.round)
            .unwrap()
            .events
            .get(&e.hash)
            .unwrap()
            .clone();

        // count votes
        let mut vote_results = HashMap::new();

        for witness_hash in prev_round.witnesses.iter() {
            let witness_round_event = prev_round.events.get(witness_hash).unwrap();
            let wit_hash = witness_round_event.hash;
            let got_witness = self.events.get_event(&wit_hash).unwrap();

            // vote
            round_event
                .votes
                .insert(wit_hash, self.see(e.clone(), got_witness.clone()));

            // collect votes
            if self.strongly_see(e.clone(), got_witness) {
                for (hash, vote) in witness_round_event.clone().votes {
                    if vote {
                        *vote_results.entry(hash).or_insert(0) += 1;
                    }
                }
            }
        }

        self.rounds
            .get_mut(&e.round)
            .unwrap()
            .events
            .insert(e.hash, round_event.clone());

        for (hash, votes) in vote_results.iter() {
            // let super_majority = self.get_decided_peers(&e).super_majority;

            // let mut rounds = self.rounds.write().unwrap();
            let mut prev_prev_round = self.rounds.get_mut(&(e.round - 2)).unwrap();
            let super_majority = prev_prev_round.peers.super_majority;

            let is_famous = if votes.clone() >= super_majority {
                FamousType::True
            } else {
                FamousType::False
            };
            prev_prev_round.events.get_mut(hash).unwrap().famous = is_famous.clone();
        }

        self.decide_round_received();
    }

    pub fn decide_round_received(&mut self) {
        // warn!("DECIDE ROUND");
        trace_time!("Decide Round Received");

        let mut decided_events = vec![];

        for (_, undecided) in self.events.undecided.clone() {
            // start at r+1

            let last_round = self.rounds.iter().last().unwrap().0.clone();
            // warn!("UNDECIDED ROUND {}", undecided.round);

            for i in undecided.round + 1..last_round {
                let round = &self.rounds.get(&i).unwrap().clone();

                let witness_iter = round
                    .witnesses
                    .iter()
                    .map(|hash| (hash, round.events.get(hash).unwrap()));

                if witness_iter
                    .clone()
                    .any(|(_, e)| e.famous == FamousType::Undefined)
                {
                    break;
                }

                let famous = witness_iter.filter(|(_, e)| e.famous == FamousType::True);

                if famous.clone().count() == 0 {
                    break;
                }

                let mut decided = true;

                for (hash, _) in famous {
                    let got_witness = self.events.get_event(&hash).unwrap();

                    if !self.see(got_witness, undecided.clone()) {
                        decided = false;

                        break;
                    }
                }

                if decided {
                    // let mut rounds = &mut self.rounds.write().unwrap();

                    self.rounds
                        .get_mut(&undecided.round)
                        .unwrap()
                        .events
                        .get_mut(&undecided.hash)
                        .unwrap()
                        .received = i as u64;

                    // (*round.events.get(&undecided.hash).unwrap().write().unwrap()).received =
                    //     i as u64;

                    decided_events.push(undecided.hash.clone());

                    break;
                }
            }
        }

        for hash in decided_events.clone() {
            self.events.undecided.remove(&hash);
        }

        trace!("Decide Round: Decided events {}", decided_events.len());
        trace!(
            "Decide Round: Undecided events {}",
            self.events.undecided.len()
        );

        if decided_events.len() > 0 {
            self.consensus_order(decided_events);
        }
    }

    pub fn consensus_order(&mut self, decided_events: Vec<EventHash>) {
        trace_time!("Consensus Order");

        let mut received = decided_events
            .iter()
            .map(|hash| {
                let event = self.events.get_event(&hash).unwrap();
                let round_borrowed = self.rounds.get(&event.round).unwrap().clone();

                let round_event = round_borrowed.events.get(&hash).unwrap();
                let round_received = self.rounds.get(&round_event.received).unwrap().clone();

                (event, round_received, round_event.clone())
            })
            .collect::<Vec<(Event, Round, RoundEvent)>>();

        received.sort_by(|(_, _, re1), (_, _, re2)| re1.received.cmp(&re2.received));

        let mut timestamped = received
            .iter()
            .map(|(e, r, re)| {
                let t = self.get_consensus_timestamp(e.clone(), r);

                self.rounds
                    .get_mut(&e.round)
                    .unwrap()
                    .events
                    .get_mut(&re.hash)
                    .unwrap()
                    .timestamp = t;
                (e.clone(), r.clone(), t)
            })
            .collect::<Vec<(Event, Round, u64)>>();

        timestamped.sort_by(|(_, _, t1), (_, _, t2)| t1.cmp(t2));

        // cleanup old events
        let max_round = timestamped
            .clone()
            .iter()
            .max_by(|(e1, _, _), (e2, _, _)| e1.round.cmp(&e2.round))
            .unwrap()
            .0
            .round
            .clone();

        //

        self.purge(max_round);

        // process tie here

        let txs = timestamped
            .iter()
            .map(|tuple| {
                (
                    tuple.0.transactions.clone(),
                    tuple.0.internal_transactions.clone(),
                    tuple.1.clone(),
                )
            })
            .collect::<Vec<(Vec<Vec<u8>>, Vec<PeerTx>, Round)>>();

        if txs.len() > 0 {
            for tx in txs.clone() {
                // classic transactions
                {
                    let out = self.tx_out.lock().unwrap();

                    for item in tx.0.clone() {
                        if item.len() > 0 {
                            out.send(item).unwrap();
                        }
                    }
                }
                {
                    if tx.1.len() > 0 {
                        let mut round = &mut tx.2.clone();

                        let last_round = self.rounds.values().last().unwrap().clone();
                        let last_round_id = last_round.id;

                        for i in last_round_id + 1..=round.id + 3 {
                            let mut r = Round::new(i);

                            r.peers = last_round.peers.clone();

                            self.rounds.insert(i, r);
                        }

                        let rounds_to_modify = self
                            .rounds
                            .iter()
                            .skip_while(|(id, _)| id < &&(round.id + 3))
                            .map(|tuple| tuple.0.clone())
                            .collect::<Vec<u64>>();

                        // peer transactions
                        for item in tx.1.clone() {
                            if item.tx_type == PeerTxType::Join {
                                for round in rounds_to_modify.clone() {
                                    self.rounds
                                        .get_mut(&round)
                                        .unwrap()
                                        .peers
                                        .add(item.peer.clone());
                                }
                            }
                            if item.tx_type == PeerTxType::Leave {
                                for round in rounds_to_modify.clone() {
                                    self.rounds
                                        .get_mut(&round)
                                        .unwrap()
                                        .peers
                                        .remove(item.peer.clone());
                                }
                            }
                            // TODO: remove peer
                        }
                    }
                }

                self.transactions.extend(tx.0);
                self.internal_transactions.extend(tx.1);
            }
        }
    }

    pub fn get_consensus_timestamp(&mut self, event: Event, round: &Round) -> u64 {
        let mut timestamps = round
            .witnesses
            .iter()
            .map(|hash| (hash, round.events.get(hash).unwrap()))
            .filter(|(_, e)| e.famous == FamousType::True)
            .map(|(_, witness)| {
                let witness_event = self.events.get_event(&witness.hash).unwrap();

                self.get_first_decendant(event.clone(), witness_event)
                    .unwrap()
                    .timestamp
            })
            .collect::<Vec<u64>>();

        timestamps.sort();

        let middle_idx = timestamps.len() / 2;
        let middle = timestamps[middle_idx];

        let before = if middle_idx > 0 {
            timestamps[middle_idx - 1]
        } else {
            middle
        };

        let after = if timestamps.len() > middle_idx + 1 {
            timestamps[middle_idx + 1]
        } else {
            middle
        };

        (before + middle + after) / 3
    }

    pub fn get_first_decendant(
        &mut self,
        event: Event,
        possible_decendant: Event,
    ) -> Option<Event> {
        let hash = (event.hash, possible_decendant.hash);

        if let Some(res) = self.first_decendant_cache.get(&hash) {
            return self.events.get_event(&res);
        }

        let res = self._get_first_decendant(event, possible_decendant);

        if let Some(res_event) = res.clone() {
            self.first_decendant_cache.insert(hash, res_event.hash);
        }

        res
    }

    pub fn _get_first_decendant(
        &mut self,
        event: Event,
        possible_decendant: Event,
    ) -> Option<Event> {
        if !self.is_ancestor(event.clone(), possible_decendant.clone()) {
            return None;
        }

        if event.hash == possible_decendant.hash {
            return Some(possible_decendant);
        }

        let self_parent = self.events.get_event(&possible_decendant.self_parent);
        let other_parent = self.events.get_event(&possible_decendant.other_parent);

        if self_parent.is_none() {
            return None;
        }

        let self_parent = self_parent.unwrap();

        if self_parent.hash == event.hash
            || self.is_self_ancestor(event.clone(), possible_decendant.clone())
        {
            return Some(possible_decendant);
        }

        if other_parent.is_some() {
            let other_parent = other_parent.unwrap();

            if other_parent.hash == event.hash {
                return Some(possible_decendant);
            }

            if let Some(e) = self._get_first_decendant(event.clone(), other_parent) {
                return Some(e);
            }
        }

        self._get_first_decendant(event, self_parent)
    }

    pub fn get_last_frame(&self, peer_id: u64) -> Frame {
        trace_time!("Get Last Frame");

        if self.get_last_decided_peers().get_by_id(peer_id).is_none() {
            return Frame::new();
        }

        let rounds_len = self.rounds.iter().last().unwrap().0.clone();

        let bound = if rounds_len <= 5 { 1 } else { rounds_len - 4 };

        let mut frame = Frame::new();

        for i in bound..=rounds_len {
            let round = self.rounds.get(&i).unwrap().clone();

            let mut creator_events = HashMap::new();

            for (e_hash, _) in round.events.iter() {
                let event = self.events.get_event(e_hash).unwrap();

                if round.peers.clone().get_by_id(event.creator).is_none() {
                    continue;
                }

                creator_events
                    .entry(event.creator)
                    .or_insert_with(|| BTreeMap::new())
                    .insert(event.id, event.clone());
            }

            frame
                .events
                .insert(i, (round.peers.clone(), creator_events));
        }

        for peer_events in frame.events.values_mut().next().unwrap().1.values_mut() {
            peer_events.iter_mut().next().unwrap().1.self_parent = 0;
        }

        frame
    }

    pub fn purge(&mut self, max_round: u64) {
        if max_round <= 5 {
            return;
        }

        trace_time!("Purge");

        let events_to_remove = self
            .rounds
            .iter_mut()
            .rev()
            .skip_while(|(id, _)| id > &&(max_round - 5))
            .map(|(_, round)| {
                if round.purged {
                    return vec![];
                }

                let hashes = round.events.keys().cloned().collect::<Vec<u64>>();

                round.purge();

                hashes
            })
            .flatten()
            .collect::<Vec<u64>>();

        self.events.purge(events_to_remove.clone());

        self.ancestor_cache = Self::purge_cache(&events_to_remove, &self.ancestor_cache);
        self.first_decendant_cache =
            Self::purge_cache(&events_to_remove, &self.first_decendant_cache);
        self.self_ancestor_cache = Self::purge_cache(&events_to_remove, &self.self_ancestor_cache);
        self.ss_cache = Self::purge_cache(&events_to_remove, &self.ss_cache);
        self.ss_path_cache = Self::purge_cache(&events_to_remove, &self.ss_path_cache);
    }

    fn purge_cache<T: Clone>(
        hash: &Vec<u64>,
        cache: &HashMap<(EventHash, EventHash), T>,
    ) -> HashMap<(EventHash, EventHash), T> {
        cache
            .iter()
            .filter(|((h1, h2), _)| hash.contains(h1) || hash.contains(h2))
            .map(|(h, b)| (h.clone(), b.clone()))
            .collect()
    }
}
