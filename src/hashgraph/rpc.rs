use std::collections::HashMap;
use std::sync::RwLock;

use super::event::EventCreator;
use super::events::EventsDiff;
use super::hashgraph::Hashgraph;
use super::node::Node;
use super::peer::Peer;
use super::peers::Peers;

service! {
  HgRpc {
    let node: Arc<super::RwLock<super::Node>>;
    let hg: Arc<super::RwLock<super::Hashgraph>>;
    let peers: Arc<super::RwLock<super::Peers>>;

    fn pull(&mut self, known: super::HashMap<super::EventCreator, u64>) -> super::EventsDiff {
      let hg = self.hg.write().unwrap();

      if let None = self.peers.read().unwrap().get_by_socket_addr(self.actual_sender.clone()) {
        warn!("Unknown peer tries to pull {}", self.actual_sender.clone());

        return super::EventsDiff::default();
      }

      trace!("Got events to pull {:?}", known);

      hg.events.events_diff(known)
    }

    fn push(&mut self, events: super::EventsDiff) -> bool {
      let mut hg = self.hg.write().unwrap();

      if let None = self.peers.read().unwrap().get_by_socket_addr(self.actual_sender.clone()) {
        warn!("Unknown peer tries to push {}", self.actual_sender.clone());

        return false;
      }

      trace!("Got events to push {:?}", events);
      let self_id = self.peers.read().unwrap().self_id;
      let peer_id = self.peers.read().unwrap().get_by_socket_addr(self.actual_sender).unwrap().id;

      hg.merge_events(self_id, peer_id, events);

      true
    }

    // you are asked to add a new peer. Answer with own pub_key
    fn ask_join(&mut self, peer: super::Peer) -> super::Peers {
      self.node.write().unwrap().peer_join(peer);

      self.peers.read().unwrap().clone()
    }
  }
}
