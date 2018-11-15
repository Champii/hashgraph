use rand;
use std::collections::BTreeMap;
use std::net::SocketAddr;

use super::peer::Peer;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Peers {
    pub self_id: u64,
    pub super_majority: u64,
    peers: BTreeMap<u64, Peer>,
    last_peer: u64,
}

impl Peers {
    pub fn new() -> Peers {
        Peers {
            self_id: 0,
            super_majority: 0,
            peers: BTreeMap::new(),
            last_peer: 0,
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

    pub fn get_self(self) -> Option<Peer> {
        match self.peers.get(&self.self_id) {
            Some(peer) => Some(peer.clone()),
            None => None,
        }
    }

    pub fn get_random(&mut self) -> Option<Peer> {
        if self.peers.len() <= 1 {
            warn!("Waiting for peers...");

            return None;
        }

        let other_peers = self.peers_without(&self.self_id);

        let peers_without_last = if self.peers.len() > 2 {
            if self.last_peer != 0 {
                self.peers_without(&self.last_peer)
            } else {
                other_peers
            }
        } else {
            other_peers
        };

        let peer_id = rand::random::<u64>() % (peers_without_last.len() as u64);

        self.last_peer = peer_id;

        let peers_vec: Vec<(&u64, &Peer)> = peers_without_last.iter().collect();

        Some(peers_vec[peer_id as usize].1.clone())
    }

    fn peers_without(&self, id: &u64) -> BTreeMap<u64, Peer> {
        let mut res = self.peers.clone();

        res.remove(id);

        res
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn merge(&mut self, peers: Peers) {
        for (_, peer) in peers.peers {
            self.add(peer);
        }
    }

    pub fn get_peers(self) -> BTreeMap<u64, Peer> {
        self.peers.clone()
    }
}
