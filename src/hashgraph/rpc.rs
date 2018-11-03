use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::event::EventCreator;
use super::events::EventsDiff;
use super::hashgraph::Hashgraph;
use super::peers::Peers;

service! {
  HgRpc {
    let hg: Arc<Mutex<super::Hashgraph>>;
    let peers: Arc<Mutex<super::Peers>>;

    fn pull(&mut self, known: super::HashMap<super::EventCreator, u64>) -> super::EventsDiff {
      trace!("Got events to pull {:?}", known);

      self.hg.lock().unwrap().events.events_diff(known)
    }

    fn push(&mut self, events: super::EventsDiff) -> bool {
      trace!("Got events to push {:?}", events);
      let self_id = self.peers.lock().unwrap().self_id;
      let peer_id = self.peers.lock().unwrap().get_by_socket_addr(self.actual_sender).unwrap().id;

      self.hg.lock().unwrap().merge_events(self_id, peer_id, events);

      true
    }
  }
}
