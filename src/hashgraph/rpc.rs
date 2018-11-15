use std::collections::HashMap;
use std::sync::RwLock;

use super::event::EventCreator;
use super::events::{EventsDiff, Frame};
use super::hashgraph::Hashgraph;
use super::internal_txs::PeerTx;
use super::node::Node;
use super::peer::Peer;
use super::peers::Peers;

service! {
  HgRpc {
    let node: Arc<super::RwLock<super::Node>>;
    let hg: Arc<super::RwLock<super::Hashgraph>>;
    let peers: Arc<super::RwLock<super::Peers>>;

    fn fast_sync(&mut self, peer_id: u64) -> super::Frame {
      self.hg.read().unwrap().get_last_frame(peer_id)
    }

    fn pull(&mut self, known: super::HashMap<super::EventCreator, u64>) -> super::EventsDiff {
      trace!("Got events to pull {:?}", known);

      self.hg.read().unwrap().events.events_diff(known, 8)
    }

    fn push(&mut self, events: super::EventsDiff) -> bool {
      let peers = self.hg.read().unwrap().get_last_decided_peers();

      let self_id = peers.clone().self_id;
      let peer = peers.clone().get_by_id(events.sender_id);

      let id = if let Some(p) = peer {
        p.id
      } else {
        warn!("UNKNOWN PEER, {:?}", events.sender_id);
        0
      };

      trace!("Got events to push {:?}", events);

      self.hg.write().unwrap().merge_events(self_id, id, events);

      true
    }

    // you are asked to add a new peer. Answer with own pub_key
    fn ask_join(&mut self, peer: super::Peer) -> bool{

      if self.hg.read().unwrap().get_last_decided_peers().len() == 1 {
          error!("HERE CA FOURE");
          let mut  hg = self.hg.write().unwrap();

          // {
          //   let rounds_len = hg.rounds.len();
          //   let mut rounds = hg.rounds.clone();
          //   let mut peers = &mut rounds.values().last().unwrap().write().unwrap().peers;

          //   peers.add(peer.clone());
          // }

          hg.add_self_event(vec![], vec![super::PeerTx::new_join(peer)]);

          hg.add_self_event(vec![], vec![]);
          hg.add_self_event(vec![], vec![]);
          hg.add_self_event(vec![], vec![]);
          hg.add_self_event(vec![], vec![]);
          hg.add_self_event(vec![], vec![]);
          hg.add_self_event(vec![], vec![]);

          return true;
      }

      self.node.write().unwrap().peer_join(peer);

      true
    }
  }
}
