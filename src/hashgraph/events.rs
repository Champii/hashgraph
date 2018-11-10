use std::collections::HashMap;

use super::event::{Event, EventCreator, EventHash};

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct EventsDiff {
    pub known: HashMap<EventCreator, u64>,
    pub diff: HashMap<EventCreator, Vec<Event>>,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Events {
    by_hash: HashMap<EventHash, Event>,
    by_creator: HashMap<EventCreator, Vec<Event>>,
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
        if !self.check_event(&event) {
            return;
        }

        self.by_hash.insert(event.hash, event.clone());
        self.undecided.insert(event.hash, event.clone());

        self.by_creator
            .entry(event.creator)
            .and_modify(|events| events.push(event.clone()))
            .or_insert(vec![event.clone()]);

        trace!("INSERT EVENT {:?}", event);
    }

    fn check_event(&self, event: &Event) -> bool {
        if let Some(_) = self.by_hash.get(&event.hash) {
            warn!("Insert event: Known event: {:?}", event);

            return false;
        }

        let creator_events = self.by_creator.get(&event.creator);

        if let Some(events) = creator_events {
            if events.len() != event.id as usize {
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

        true
    }

    pub fn known_events(&self) -> HashMap<EventCreator, u64> {
        let mut res = HashMap::new();

        for (peer_id, events) in &self.by_creator {
            res.insert(peer_id.clone(), events[events.len() - 1].id);
        }

        trace!("KNOWN {:?}", res);

        res
    }

    pub fn events_diff(&self, other_known: HashMap<EventCreator, u64>) -> EventsDiff {
        let mut res = HashMap::new();
        let known = self.known_events();

        trace!("OTHER KNOWN {:?}", other_known);

        for (peer_id, last_known) in &known {
            match other_known.get(peer_id) {
                Some(other_last_known) => {
                    if other_last_known < last_known {
                        let events = self.by_creator.get(peer_id).unwrap();
                        let mut to_add = vec![];

                        for i in other_last_known.clone() + 1..=last_known.clone() {
                            to_add.push(events[i as usize].clone());
                        }

                        res.insert(peer_id.clone(), to_add);
                    }
                }
                None => {
                    let events = self.by_creator.get(peer_id).unwrap();

                    res.insert(peer_id.clone(), events.clone());
                }
            }
        }

        trace!("DIFF {:?}", res);

        EventsDiff { known, diff: res }
    }

    pub fn get_last_event_of(&self, creator: EventCreator) -> Option<Event> {
        self.by_creator
            .get(&creator)
            .map(|events| events[events.len() - 1].clone())
    }

    pub fn get_event(&self, hash: EventHash) -> Option<Event> {
        self.by_hash.get(&hash).map(|event| event.clone())
    }
}
