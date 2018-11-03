use rand;
use std::collections::HashMap;
use std::net::SocketAddr;

use super::peer::Peer;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Peers {
    pub self_id: u64,
    pub super_majority: u64,
    peers: HashMap<u64, Peer>,
}

impl Peers {
    pub fn new() -> Peers {
        Peers {
            self_id: 0,
            super_majority: 0,
            peers: HashMap::new(),
        }
    }

    pub fn add_self(&mut self, peer: Peer) {
        self.self_id = peer.id;

        self.add(peer);
    }

    pub fn add(&mut self, peer: Peer) {
        if self.peers.get(&peer.id).is_some() {
            return;
        }

        if peer.id != self.self_id {
            info!("New peer -> {}", peer.clone());
        }

        self.peers.insert(peer.id, peer);

        self.super_majority = (2 * self.peers.len() / 3 + 1) as u64;
    }

    pub fn get_by_id(self, id: u64) -> Option<Peer> {
        match self.peers.get(&id) {
            Some(peer) => Some(peer.clone()),
            None => None,
        }
    }

    pub fn get_by_socket_addr(&self, addr: SocketAddr) -> Option<Peer> {
        for (_, peer) in self.peers.iter() {
            if peer.address == addr {
                return Some(peer.clone());
            }
        }

        None
    }

    pub fn get_random(self) -> Option<Peer> {
        if self.peers.len() <= 1 {
            warn!("Waiting for peers...");

            return None;
        }

        let other_peers = self.peers_without_self();

        let peer_id = rand::random::<u8>() % (other_peers.len() as u8);

        let peers_vec: Vec<(&u64, &Peer)> = other_peers.iter().collect();

        Some(peers_vec[peer_id as usize].1.clone())
    }

    fn peers_without_self(self) -> HashMap<u64, Peer> {
        let mut res = self.peers.clone();

        res.remove(&self.self_id);

        res
    }
}
