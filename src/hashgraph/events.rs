use std::collections::{BTreeMap, HashMap};

use super::event::{Event, EventCreator, EventHash};
use peers::Peers;

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Frame {
    pub events: BTreeMap<u64, (Peers, HashMap<EventCreator, BTreeMap<u64, Event>>)>, // round_id -> (peers, (creator_id -> (event_id, event)))
}

impl Frame {
    pub fn new() -> Frame {
        Frame {
            events: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct EventsDiff {
    pub known: HashMap<EventCreator, u64>,
    pub diff: HashMap<EventCreator, BTreeMap<u64, Event>>, // creator -> (id, event),
    pub sender_id: EventCreator,
    pub has_more: bool,
    // todo: add signature
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Events {
    by_hash: HashMap<EventHash, Event>,
    by_creator: HashMap<EventCreator, BTreeMap<u64, Event>>, // creator -> (id, event)
    pub undecided: HashMap<EventHash, Event>,
}

impl Events {
    pub fn new() -> Events {
        Events {
            by_hash: HashMap::new(),
            by_creator: HashMap::new(),
            undecided: HashMap::new(),
        }
    }

    pub fn insert_event(&mut self, event: Event) {
        self.by_hash.insert(event.hash, event.clone());
        self.undecided.insert(event.hash, event.clone());

        self.by_creator
            .entry(event.creator)
            .and_modify(|events| {
                events.insert(event.id, event.clone());
            })
            .or_insert_with(|| {
                let mut res = BTreeMap::new();

                res.insert(event.id, event.clone());

                res
            });

        trace!("INSERT EVENT {:?}", event);
    }

    pub fn check_event(&self, event: &Event) -> bool {
        if let Some(_) = self.by_hash.get(&event.hash) {
            debug!("Insert event: Known event: {:?}", event);

            return false;
        }

        let creator_events = self.by_creator.get(&event.creator);

        if let Some(events) = creator_events {
            if events.len() > 0 && events.values().last().unwrap().id + 1 != event.id {
                error!("HERE {:?}", events);
                error!("Insert event: Non-sequential event: {:?}", event);

                return false;
            }
        }

        match self.by_hash.get(&event.self_parent) {
            Some(e) => {
                if e.creator != event.creator {
                    error!("Insert event: Bad event self-parent {:?}", event);

                    return false;
                }
            }
            None => {
                if let Some(arr) = creator_events {
                    if arr.len() > 0 {
                        error!("Insert event: Bad event self-parent is nil {:?}", event);

                        return false;
                    }
                }
            }
        }

        // if event.other_parent != 0 {
        //     match self.by_hash.get(&event.other_parent) {
        //         Some(e) => {
        //             return true;
        //         }
        //         None => {
        //             if let Some(arr) = creator_events {
        //                 if arr.len() > 0 {
        //                     error!("Insert event: Bad event other-parent is nil {:?}", event);

        //                     return false;
        //                 }
        //             }
        //         }
        //     }
        // }

        true
    }

    pub fn known_events(&self) -> HashMap<EventCreator, u64> {
        let mut res = HashMap::new();

        for (peer_id, events) in &self.by_creator {
            res.insert(peer_id.clone(), events.values().last().unwrap().id);
        }

        trace!("KNOWN {:?}", res);

        res
    }

    pub fn events_diff(&self, other_known: HashMap<EventCreator, u64>, limit: u64) -> EventsDiff {
        let mut res_events = HashMap::new();
        let known = self.known_events();
        // let mut res_known = HashMap::new();
        let mut has_more = false;

        trace!("OTHER KNOWN {:?}", other_known);

        for (peer_id, last_known) in &known {
            match other_known.get(peer_id) {
                Some(other_last_known) => {
                    let mut other_last_known = other_last_known.clone();
                    let mut last_known = last_known.clone();

                    if other_last_known < last_known.clone() {
                        if limit > 0 && other_last_known + limit < last_known {
                            warn!("LIMIT !!!!, {}", last_known - other_last_known);
                            has_more = true;
                            last_known = other_last_known + limit;
                        }

                        let events = self.by_creator.get(peer_id).unwrap();
                        let mut to_add = BTreeMap::new();

                        for i in other_last_known.clone()..=last_known.clone() {
                            let mut event = events.get(&i);

                            if event.is_none() {
                                continue;
                            }

                            let mut event = event.unwrap();

                            to_add.insert(event.id, event.clone());
                        }

                        res_events.insert(peer_id.clone(), to_add);
                        // res_known.insert(peer_id.clone(), last_known.clone());
                    }
                }
                None => {
                    let mut events = self.by_creator.get(peer_id).unwrap().clone();

                    // if limit > 0 && events.len() > limit as usize {
                    //     events.truncate(limit as usize);

                    //     has_more = true;
                    // }

                    // res_known.insert(peer_id.clone(), events.len() as u64);
                    res_events.insert(peer_id.clone(), events.clone());
                }
            }
        }

        trace!("DIFF {:?}", res_events);

        EventsDiff {
            sender_id: 0,
            known: known,
            diff: res_events,
            has_more,
        }
    }

    pub fn get_last_event_of(&self, creator: EventCreator) -> Option<Event> {
        self.by_creator
            .get(&creator)
            .map(|events| events.values().last().unwrap().clone())
    }

    pub fn get_event(&self, hash: &EventHash) -> Option<Event> {
        self.by_hash.get(hash).map(|event| event.clone())
    }
}
