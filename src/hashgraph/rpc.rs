use std::collections::HashMap;
use std::sync::RwLock;

use super::event::EventCreator;
use super::events::EventsDiff;
use super::hashgraph::Hashgraph;
use super::peers::Peers;

service! {
  HgRpc {
    let hg: Arc<super::RwLock<super::Hashgraph>>;
    let peers: Arc<super::RwLock<super::Peers>>;

    fn pull(&mut self, known: super::HashMap<super::EventCreator, u64>) -> super::EventsDiff {
      trace!("Got events to pull {:?}", known);

      self.hg.read().unwrap().events.events_diff(known)
    }

    fn push(&mut self, events: super::EventsDiff) -> bool {
      trace!("Got events to push {:?}", events);
      let self_id = self.peers.read().unwrap().self_id;
      let peer_id = self.peers.read().unwrap().get_by_socket_addr(self.actual_sender).unwrap().id;

      self.hg.write().unwrap().merge_events(self_id, peer_id, events);

      true
    }
  }
}
