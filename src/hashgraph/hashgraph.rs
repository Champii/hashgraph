use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};

use super::event::{Event, EventCreator, EventHash};
use super::events::{Events, EventsDiff};
use super::peers::Peers;
use super::round::{FamousType, Round, RoundEvent};

#[derive(Debug)]
pub struct Hashgraph {
    pub peers: Arc<RwLock<Peers>>,
    pub events: Events,
    pub rounds: Vec<Arc<RwLock<Round>>>,
    pub tx_out: Mutex<Sender<Vec<u8>>>,
    pub transactions: Vec<Vec<u8>>,
}

impl Default for Hashgraph {
    fn default() -> Hashgraph {
        let (tx_out, _) = channel();

        Hashgraph::new(Arc::new(RwLock::new(Peers::new())), Mutex::new(tx_out))
    }
}

impl Hashgraph {
    pub fn new(peers: Arc<RwLock<Peers>>, tx_out: Mutex<Sender<Vec<u8>>>) -> Hashgraph {
        Hashgraph {
            peers,
            events: Events::new(),
            rounds: vec![Arc::new(RwLock::new(Round::new(1)))], // rounds start at 1
            transactions: vec![],
            tx_out,
        }
    }

    pub fn add_transaction(&mut self, tx: Vec<u8>) {
        let self_id = self.peers.read().unwrap().self_id;

        let last_own_event = self.events.get_last_event_of(self_id).unwrap();

        self.insert_event(Event::new(
            last_own_event.id + 1,
            self_id,
            last_own_event.hash,
            0,
            vec![tx],
        ));
    }

    pub fn insert_event(&mut self, event: Event) {
        let mut e = event.clone();

        let round_id = self.set_round(event.clone());

        e.round = round_id;

        self.events.insert_event(e.clone());
        self.process_fame(e.clone());
    }

    pub fn merge_events(
        &mut self,
        self_id: u64,
        peer_id: u64,
        other_events: EventsDiff,
    ) -> EventsDiff {
        for (_, events) in other_events.diff {
            for event in events {
                self.insert_event(event);
            }
        }
        // self.events.merge_events(other_events.clone());

        let last_own_event = self.events.get_last_event_of(self_id).unwrap();

        let last_other_event = self.events.get_last_event_of(peer_id).unwrap();

        self.insert_event(Event::new(
            last_own_event.id + 1,
            self_id,
            last_own_event.hash,
            last_other_event.hash,
            vec![],
        ));

        self.events.events_diff(other_events.known)
    }

    pub fn is_ancestor(&self, possible_ancestor: Event, e: Event) -> bool {
        if possible_ancestor.hash == e.hash {
            return true;
        }

        let self_parent = self.events.get_event(e.self_parent);
        let other_parent = self.events.get_event(e.other_parent);

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

    pub fn is_self_ancestor(&self, possible_ancestor: Event, e: Event) -> bool {
        let self_parent = self.events.get_event(e.self_parent);

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

    pub fn see(&self, e: Event, possible_see: Event) -> bool {
        self.is_ancestor(possible_see, e)
    }

    pub fn strongly_see(&self, e: Event, possible_see: Event) -> bool {
        let super_majority = self.peers.read().unwrap().super_majority;

        let res = self.strongly_see_with_path(e, possible_see);

        res.0 == true && res.1.len() >= super_majority as usize
    }

    fn strongly_see_with_path(&self, e: Event, possible_see: Event) -> (bool, Vec<EventCreator>) {
        let self_parent = self.events.get_event(e.self_parent);
        let other_parent = self.events.get_event(e.other_parent);

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

    pub fn is_witness(&self, e: Event) -> bool {
        if e.is_root() {
            return true;
        }

        let last_round = self.get_parent_round(e.clone());

        let mut ss_count = 0;

        for (_, witness) in last_round.read().unwrap().witnesses.iter() {
            let got_witness = self.events.get_event(witness.read().unwrap().hash).unwrap();

            if self.strongly_see(e.clone(), got_witness) {
                ss_count += 1;
            }
        }

        ss_count >= self.peers.read().unwrap().super_majority
    }

    pub fn set_round(&mut self, e: Event) -> u64 {
        let is_witness = self.is_witness(e.clone());

        let mut last_round = self.get_parent_round(e.clone());
        let mut last_round_id = last_round.read().unwrap().id;

        if !e.is_root() && is_witness {
            last_round_id += 1;
        }

        if self.rounds.len() == last_round_id as usize - 1 {
            // new round
            let mut round = Round::new(last_round_id);

            round.insert(e.clone(), is_witness);

            self.rounds.push(Arc::new(RwLock::new(round)));
        } else {
            // add to current round
            (*self.rounds[last_round_id as usize - 1].write().unwrap())
                .insert(e.clone(), is_witness);
        }

        last_round_id
    }

    pub fn get_parent_round(&self, e: Event) -> Arc<RwLock<Round>> {
        let self_parent = self.events.get_event(e.self_parent);
        let other_parent = self.events.get_event(e.other_parent);

        let mut round = 1;

        if self_parent.is_some() {
            round = self_parent.unwrap().round
        } else if other_parent.is_some() {
            let other_round = other_parent.unwrap().round;

            if other_round > round {
                round = other_round
            }
        }

        self.rounds[round as usize - 1].clone()
    }

    pub fn process_fame(&mut self, e: Event) {
        if e.round == 1 || !self.is_witness(e.clone()) {
            return;
        }

        let round = self.rounds[e.round as usize - 1].clone();
        let round_event = round.read().unwrap().events.get(&e.hash).unwrap().clone();

        let prev_round = self.rounds[e.round as usize - 2].clone();

        // count votes
        let mut vote_results = HashMap::new();

        for (_, witness) in prev_round.read().unwrap().witnesses.iter() {
            let wit_hash = witness.read().unwrap().hash;
            let got_witness = self.events.get_event(wit_hash).unwrap();

            // vote
            (*round_event.write().unwrap())
                .votes
                .insert(wit_hash, self.see(e.clone(), got_witness.clone()));

            // collect votes
            if self.strongly_see(e.clone(), got_witness) {
                for (hash, vote) in witness.read().unwrap().clone().votes {
                    if vote {
                        (*vote_results.entry(hash).or_insert(0)) += 1;
                    }
                }
            }
        }

        for (hash, votes) in vote_results.iter() {
            let prev_prev_round = self.rounds[e.round as usize - 3].read().unwrap();
            if votes.clone() >= self.peers.read().unwrap().super_majority {
                (*(*prev_prev_round)
                    .events
                    .get(hash)
                    .unwrap()
                    .write()
                    .unwrap())
                .famous = FamousType::True;
            } else {
                (*(*prev_prev_round)
                    .events
                    .get(hash)
                    .unwrap()
                    .write()
                    .unwrap())
                .famous = FamousType::False;
            }
        }

        self.decide_round_received();
    }

    pub fn decide_round_received(&mut self) {
        let mut decided_events = vec![];

        for (_, undecided) in &self.events.undecided {
            // start at r+1
            for i in undecided.round as usize..self.rounds.len() {
                let round = &self.rounds[i].read().unwrap();

                let witness_iter = round.witnesses.iter();

                if witness_iter
                    .clone()
                    .any(|(_, e)| e.read().unwrap().famous == FamousType::Undefined)
                {
                    break;
                }

                let famous =
                    witness_iter.filter(|(_, e)| e.read().unwrap().famous == FamousType::True);

                if famous.clone().count() == 0 {
                    break;
                }

                let mut decided = true;

                for (hash, _) in famous {
                    let got_witness = self.events.get_event(hash.clone()).unwrap();

                    if !self.see(got_witness, undecided.clone()) {
                        decided = false;

                        break;
                    }
                }

                if decided {
                    let round = self.rounds[undecided.round as usize - 1].read().unwrap();

                    (*round.events.get(&undecided.hash).unwrap().write().unwrap()).received =
                        i as u64 + 1;

                    decided_events.push(undecided.hash.clone());

                    break;
                }
            }
        }

        for hash in decided_events.clone() {
            self.events.undecided.remove(&hash);
        }

        if decided_events.len() > 0 {
            self.consensus_order(decided_events);
        }
    }

    pub fn consensus_order(&mut self, decided_events: Vec<EventHash>) {
        //
        let mut received = decided_events
            .iter()
            .map(|hash| {
                let event = self.events.get_event(hash.clone()).unwrap();
                let round_borrowed = self.rounds[event.round as usize - 1].read().unwrap();
                let round_event = round_borrowed.events.get(&hash).unwrap();
                let round_received =
                    self.rounds[round_event.read().unwrap().received as usize - 1].clone();

                (event, round_received, round_event.clone())
            })
            .collect::<Vec<(Event, Arc<RwLock<Round>>, Arc<RwLock<RoundEvent>>)>>();

        received.sort_by(|(e1, r1, re1), (e2, r2, re2)| {
            re1.read()
                .unwrap()
                .received
                .cmp(&re2.read().unwrap().received)
        });

        let mut timestamped = received
            .iter()
            .map(|(e, r, re)| {
                let t = self.get_consensus_timestamp(e.clone(), r);

                re.write().unwrap().timestamp = t;

                (e.clone(), t)
            })
            .collect::<Vec<(Event, u64)>>();

        timestamped.sort_by(|(e1, t1), (e2, t2)| t1.cmp(t2));

        let txs = timestamped
            .iter()
            .map(|tuple| tuple.0.transactions.clone())
            .collect::<Vec<Vec<Vec<u8>>>>()
            .concat();

        if txs.len() > 0 {
            for tx in txs.clone() {
                self.tx_out.lock().unwrap().send(tx);
            }

            self.transactions.extend(txs);
        }
    }

    pub fn get_consensus_timestamp(&mut self, event: Event, round: &Arc<RwLock<Round>>) -> u64 {
        let mut timestamps = round
            .read()
            .unwrap()
            .witnesses
            .iter()
            .filter(|(_, e)| e.read().unwrap().famous == FamousType::True)
            .map(|(_, witness)| {
                let witness_event = self.events.get_event(witness.read().unwrap().hash).unwrap();

                self.get_first_decendant(event.clone(), witness_event)
                    .unwrap()
                    .timestamp
            })
            .collect::<Vec<u64>>();

        timestamps.sort();

        timestamps[(timestamps.len() / 2)]
    }

    pub fn get_first_decendant(&self, event: Event, possible_decendant: Event) -> Option<Event> {
        if event.hash == possible_decendant.hash {
            return Some(possible_decendant);
        }

        let self_parent = self.events.get_event(possible_decendant.self_parent);
        let other_parent = self.events.get_event(possible_decendant.other_parent);

        if self_parent.is_none() {
            return None;
        }

        let self_parent = self_parent.unwrap();

        if self_parent.hash == event.hash {
            return Some(possible_decendant);
        }

        if other_parent.is_some() {
            let other_parent = other_parent.unwrap();

            if other_parent.hash == event.hash {
                return Some(possible_decendant);
            }

            if let Some(e) = self.get_first_decendant(event.clone(), other_parent) {
                return Some(e);
            }
        }

        self.get_first_decendant(event, self_parent)
    }
}

mod tests {
    use std::collections::HashMap;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex, RwLock};

    use super::Event;
    use super::FamousType;
    use super::Hashgraph;
    use super::Peers;
    use peer::Peer;

    // new_hash, other_parent
    type EventInsert = (String, String, String);

    #[allow(dead_code)]
    fn insert_events(
        to_insert: Vec<EventInsert>,
        peers: Peers,
    ) -> (Hashgraph, HashMap<String, Event>) {
        let mut indexes = HashMap::new();
        let (tx_out, tx_out_receiver) = channel();

        let mut hg = Hashgraph::new(Arc::new(RwLock::new(peers)), Mutex::new(tx_out));

        for event in to_insert.iter() {
            let event_hash_bytes = event.0.as_bytes();
            let peer = event_hash_bytes[0];
            let idx = event_hash_bytes[1];

            let self_hash: &[u8] = &(if idx == 0 { [peer, 0] } else { [peer, idx - 1] });

            let self_p = indexes
                .get(&String::from_utf8(self_hash.to_vec()).unwrap())
                .map_or(0, |ev: &Event| ev.hash);

            let other_p = indexes.get(&event.1).map_or(0, |ev: &Event| ev.hash);

            let tx = event.2.clone().into_bytes();
            let mut txs = vec![];

            if tx.len() > 0 {
                txs = vec![tx];
            }

            let e = Event::new((idx - 48) as u64, (peer - 97) as u64, self_p, other_p, txs);

            indexes.insert(event.0.clone(), e.clone());

            // println!("INSERT {}", event.0);

            hg.insert_event(e)
        }

        (hg, indexes)
    }

    /*
        b1
       /|
      / |
    a1  |
    | \ |
    |  \|
    a0  b0
    */

    #[test]
    fn simple_test() {
        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());

        // (name, other_parent)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string()),
            ("b0".to_string(), "".to_string(), "".to_string()),
            ("a1".to_string(), "b0".to_string(), "".to_string()),
            ("b1".to_string(), "a1".to_string(), "".to_string()),
        ];

        let (hg, indexes) = insert_events(to_insert, peers);

        // ancestor
        let assert_ancestor = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.is_ancestor(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };
        let assert_self_ancestor = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.is_self_ancestor(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_strongly_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.strongly_see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_witness = |hash: &str, res: bool| {
            assert_eq!(hg.is_witness(indexes.get(hash).unwrap().clone()), res);
        };

        let assert_round = |hash: &str, res: u64| {
            assert_eq!(
                hg.events
                    .get_event(indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .round,
                res
            );
        };

        let assert_first_decendant = |hash1: &str, hash2: &str, res: &str| {
            let ev1 = hg
                .events
                .get_event(indexes.get(hash1).unwrap().clone().hash)
                .unwrap();

            let ev2 = hg
                .events
                .get_event(indexes.get(hash2).unwrap().clone().hash)
                .unwrap();

            let ev_res = hg
                .events
                .get_event(indexes.get(res).unwrap().clone().hash)
                .unwrap();

            assert_eq!(hg.get_first_decendant(ev1, ev2).unwrap().hash, ev_res.hash);
        };

        assert_ancestor("a0", "a1", true);
        assert_ancestor("b0", "a1", true);
        assert_ancestor("a0", "b1", true);
        assert_ancestor("b0", "b1", true);
        assert_ancestor("a1", "b1", true);

        assert_ancestor("b0", "a0", false);
        assert_ancestor("b1", "a0", false);

        // self ancestor

        assert_self_ancestor("a0", "a1", true);
        assert_self_ancestor("b0", "b1", true);

        assert_self_ancestor("a0", "b1", false);
        assert_self_ancestor("b0", "a1", false);

        // see

        assert_see("a1", "a0", true);
        assert_see("a1", "b0", true);
        assert_see("b1", "a0", true);
        assert_see("b1", "b0", true);
        assert_see("b1", "a1", true);

        assert_see("a0", "b0", false);
        assert_see("a0", "b1", false);

        assert_see("b0", "a0", false);
        assert_see("b0", "a1", false);
        assert_see("b0", "b1", false);

        assert_see("a0", "b0", false);
        assert_see("a0", "a1", false);
        assert_see("a0", "b1", false);

        // strongly see

        assert_strongly_see("a1", "b0", true);
        assert_strongly_see("b1", "a0", true);
        assert_strongly_see("b1", "a1", true);
        assert_strongly_see("b1", "b0", true);

        assert_strongly_see("a1", "a0", false);

        // witness

        assert_witness("a0", true);
        assert_witness("b0", true);
        assert_witness("a1", false);
        assert_witness("b1", true);

        // rounds

        assert_round("a0", 1);
        assert_round("b0", 1);
        assert_round("a1", 1);
        assert_round("b1", 2);

        assert_first_decendant("a0", "b1", "a1");
        assert_first_decendant("a1", "b1", "b1");
        assert_first_decendant("b1", "b1", "b1");
        assert_first_decendant("b0", "b1", "b1");
    }

    /*
    |   b4  |
    |   |   |
    |   b3  |
    |  /|   | ---- r1
    | / b2  |
    |/  |   |
    a2  |   |
    | \ |   |
    |   \   |
    |   | \ |
    a1  |   c2
    |   | / |
    |   b1  c1
    | / |   |
    a0  b0  c0
    0   1    2
    */

    #[test]
    fn complex_test() {
        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);
        let peer3 = Peer::new("127.0.0.1:3".parse().unwrap(), vec![2]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());
        peers.add(peer3.clone());

        // (name, peer, index, self, other)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string()),
            ("b0".to_string(), "".to_string(), "".to_string()),
            ("c0".to_string(), "".to_string(), "".to_string()),
            ("b1".to_string(), "a0".to_string(), "".to_string()),
            ("c1".to_string(), "".to_string(), "".to_string()),
            ("c2".to_string(), "b1".to_string(), "".to_string()),
            ("a1".to_string(), "".to_string(), "".to_string()),
            ("a2".to_string(), "c2".to_string(), "".to_string()),
            ("b2".to_string(), "".to_string(), "".to_string()),
            ("b3".to_string(), "a2".to_string(), "".to_string()),
            ("b4".to_string(), "".to_string(), "".to_string()),
        ];

        let (hg, indexes) = insert_events(to_insert, peers);

        let assert_witness = |hash: &str, res: bool| {
            assert_eq!(hg.is_witness(indexes.get(hash).unwrap().clone()), res);
        };

        let assert_strongly_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.strongly_see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_round = |hash: &str, res: u64| {
            assert_eq!(
                hg.events
                    .get_event(indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .round,
                res
            );
        };

        let assert_first_decendant = |hash1: &str, hash2: &str, res: &str| {
            let ev1 = hg
                .events
                .get_event(indexes.get(hash1).unwrap().clone().hash)
                .unwrap();

            let ev2 = hg
                .events
                .get_event(indexes.get(hash2).unwrap().clone().hash)
                .unwrap();

            let ev_res = hg
                .events
                .get_event(indexes.get(res).unwrap().clone().hash)
                .unwrap();

            assert_eq!(hg.get_first_decendant(ev1, ev2).unwrap().hash, ev_res.hash);
        };

        // witness

        assert_witness("a0", true);
        assert_witness("b0", true);
        assert_witness("c0", true);
        assert_witness("a1", false);
        assert_witness("b1", false);
        assert_witness("c1", false);
        assert_witness("b3", true);

        // strongly see

        assert_strongly_see("b3", "a0", true);
        assert_strongly_see("b3", "b0", true);
        assert_strongly_see("b3", "c0", true);

        // rounds

        assert_round("a0", 1);
        assert_round("b0", 1);
        assert_round("c0", 1);
        assert_round("a1", 1);
        assert_round("b1", 1);
        assert_round("c1", 1);
        assert_round("b3", 2);
        assert_round("b4", 2);

        assert_first_decendant("a1", "b3", "a2");
    }

    /*
                      Round 5
    		|   |   c9
    		|   | / |
    		|   b9  |
    ------- |  /|   | --------------------------------
    		a9  |   | Round 4
    		| \ |   |
    		|   \   |
    		|   | \ |
    		|   |   c8
    		|   | / |
    		|   b8  |
    		| / |   |
    		a8  |   c7
    		| \ | / |
    		|   b7  |
    ------- |  /|   | --------------------------------
    		a7  |   | Round 3
    		| \ |   |
    		|   \   |
    		|   | \ |
    	    |   |   c6
    		|   | / |
    		|   b6  |
    		| / |   |
    		a6  |   c5
    		| \ | / |
    		|   b5  |
    ------- |  /|   | -------------------------------
    		a5  |   |  Round 2           
    		|   |   |                    
    		a4  |   |                    
    		| \ |   |                    
    		|   \   |                    
    		|   | \ |
    	--- a3  |   c4 //a3's other-parent is c1. This situation can happen with concurrency
    	|	|   | / |
    	|	|   b4  |
    	|	| / |   |
    	|	a2  |   c3
    	|	| \ | / |
    	|	|   b3  |
    	|	|   |   |
    	|	|   b2  |
    ----| --|  /|   | ------------------------------
    	|	a1  |   |  Round 1          
    	|	| \ |   |                   
    	|	|   \   |                   
    	|	|   | \ |                   
    	|   |   |   c2                  
    	|	|   |   |
    	----------  c1
    		|   | / |
    		|   b1  |
    	    | / |   |
    		a0  b0  c0
    		0   1    2
    */
    #[test]
    fn consensus_test() {
        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);
        let peer3 = Peer::new("127.0.0.1:3".parse().unwrap(), vec![2]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());
        peers.add(peer3.clone());

        // (name, other_parent)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string()),
            ("b0".to_string(), "".to_string(), "".to_string()),
            ("c0".to_string(), "".to_string(), "".to_string()),
            ("b1".to_string(), "a0".to_string(), "".to_string()),
            ("c1".to_string(), "b1".to_string(), "c1".to_string()),
            ("c2".to_string(), "".to_string(), "".to_string()),
            ("a1".to_string(), "c2".to_string(), "".to_string()),
            ("b2".to_string(), "a1".to_string(), "".to_string()),
            ("b3".to_string(), "".to_string(), "b3".to_string()),
            ("c3".to_string(), "b3".to_string(), "".to_string()),
            ("a2".to_string(), "b3".to_string(), "".to_string()),
            ("b4".to_string(), "a2".to_string(), "".to_string()),
            ("c4".to_string(), "b4".to_string(), "".to_string()),
            ("a3".to_string(), "c1".to_string(), "".to_string()),
            ("a4".to_string(), "c4".to_string(), "".to_string()),
            ("a5".to_string(), "".to_string(), "a5".to_string()),
            ("b5".to_string(), "a5".to_string(), "".to_string()),
            ("a6".to_string(), "b5".to_string(), "b6".to_string()),
            ("c5".to_string(), "b5".to_string(), "".to_string()),
            ("b6".to_string(), "a6".to_string(), "".to_string()),
            ("c6".to_string(), "b6".to_string(), "".to_string()),
            ("a7".to_string(), "c6".to_string(), "a7".to_string()),
            ("b7".to_string(), "a7".to_string(), "".to_string()),
            ("c7".to_string(), "b7".to_string(), "".to_string()),
            ("a8".to_string(), "b7".to_string(), "".to_string()),
            ("b8".to_string(), "a8".to_string(), "".to_string()),
            ("c8".to_string(), "b8".to_string(), "".to_string()),
            ("a9".to_string(), "c8".to_string(), "".to_string()),
            ("b9".to_string(), "a9".to_string(), "".to_string()),
            ("c9".to_string(), "b9".to_string(), "".to_string()),
        ];

        let (hg, indexes) = insert_events(to_insert, peers);

        let assert_round = |hash: &str, res: u64| {
            assert_eq!(
                hg.events
                    .get_event(indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .round,
                res
            );
        };

        let assert_famous = |r: u64, hash: &str, res: FamousType| {
            let round = hg.rounds[(r - 1) as usize].read().unwrap();

            assert_eq!(
                round
                    .witnesses
                    .get(&indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .read()
                    .unwrap()
                    .famous,
                res
            );
        };

        let assert_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_strongly_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.strongly_see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        // Rounds

        let rounds: Vec<(&str, u64)> = vec![
            ("a0", 1),
            ("b0", 1),
            ("c0", 1),
            ("b1", 1),
            ("c1", 1),
            ("c2", 1),
            ("a1", 1),
            ("b2", 2),
            ("b3", 2),
            ("a2", 2),
            ("c3", 2),
            ("b4", 2),
            ("a3", 2),
            ("c4", 2),
            ("a4", 2),
            ("a5", 2),
            ("b5", 3),
            ("a6", 3),
            ("c5", 3),
            ("b6", 3),
            ("c6", 3),
            ("a7", 3),
            ("b7", 4),
            ("a8", 4),
            ("c7", 4),
            ("b8", 4),
            ("c8", 4),
            ("a9", 4),
            ("b9", 5),
            ("c9", 5),
        ];

        for (hash, round) in rounds.iter() {
            assert_round(hash, round.clone());
        }

        // Fame

        assert_see("b2", "a0", true);
        assert_see("b2", "b0", true);
        assert_see("b2", "c0", true);

        assert_see("a2", "a0", true);
        assert_see("a2", "b0", true);
        assert_see("a2", "c0", true);

        assert_see("c3", "a0", true);
        assert_see("c3", "b0", true);
        assert_see("c3", "c0", true);

        assert_see("b5", "b2", true);
        assert_see("b5", "b2", true);
        assert_see("b5", "b2", true);
        assert_see("a6", "b2", true);
        assert_see("a6", "b2", true);
        assert_see("a6", "b2", true);
        assert_see("c5", "b2", true);
        assert_see("c5", "b2", true);
        assert_see("c5", "b2", true);

        assert_see("c0", "b6", false);

        let famous: Vec<(&str, u64, FamousType)> = vec![
            ("a0", 1, FamousType::True),
            ("b0", 1, FamousType::True),
            ("c0", 1, FamousType::True),
            ("b2", 2, FamousType::True),
            ("a2", 2, FamousType::True),
            ("c3", 2, FamousType::True),
            ("b5", 3, FamousType::True),
            ("a6", 3, FamousType::True),
            ("c5", 3, FamousType::True),
        ];

        for (hash, r, val) in famous {
            assert_famous(r, hash, val);
        }

        let undecideds = vec![
            "b5", "b6", "b7", "b8", "b9", "a6", "a7", "a8", "a9", "c5", "c6", "c7", "c8", "c9",
        ];

        //TODO: check that every undecided is in the hg.events.undecided

        assert_eq!(hg.events.undecided.len(), undecideds.len());

        let decided = vec![7, 9, 0, 0, 0];

        for (round, &count) in decided.iter().enumerate() {
            assert_eq!(
                hg.rounds[round]
                    .read()
                    .unwrap()
                    .events
                    .iter()
                    .filter(|(_, val)| val.read().unwrap().received > 0)
                    .count(),
                count,
            );
        }

        // TODO check the round received

        assert_eq!(hg.transactions[0], "c1".to_string().into_bytes());
        assert_eq!(hg.transactions[1], "b3".to_string().into_bytes());
        assert_eq!(hg.transactions[2], "a5".to_string().into_bytes());
    }
}
