use std::net::SocketAddr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use std::{thread, time};

use event::Event;
use hashgraph::Hashgraph;
use internal_txs::{PeerTx, PeerTxType};
use key::Key;
use peer::Peer;
use peers::Peers;
use rpc::HgRpc;

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub verbose: u8,
    pub listen_addr: SocketAddr,
    pub connect_addr: Option<SocketAddr>,
}

impl Default for NodeConfig {
    fn default() -> NodeConfig {
        NodeConfig {
            verbose: 2,
            listen_addr: "127.0.0.1:3000".parse().unwrap(),
            connect_addr: None,
        }
    }
}

#[derive(Clone)]
pub struct Node {
    key: Key,
    pub config: NodeConfig,
    pub peers: Arc<RwLock<Peers>>,
    // hg: Arc<Mutex<Hashgraph>>,
    // pub tx_channel: Receiver<Vec<u8>>,
    pub tx_channel: Option<Arc<Mutex<Sender<Vec<u8>>>>>,
    pub peer_channel: Option<Arc<Mutex<Sender<PeerTx>>>>,
}

impl Default for Node {
    fn default() -> Node {
        Node::new(Key::new_generate().unwrap(), NodeConfig::default())
    }
}

impl Node {
    pub fn new(key: Key, config: NodeConfig) -> Node {
        let peers = Arc::new(RwLock::new(Peers::new()));

        Node {
            key,
            config,
            peers: peers.clone(),
            // hg: Arc::new(Mutex::new(Hashgraph::new(peers, sender))),
            // tx_channel: receiver,
            tx_channel: None,
            peer_channel: None,
        }
    }

    pub fn run(&mut self) -> Receiver<Vec<u8>> {
        let (tx_out, tx_out_receiver) = channel();
        let (tx_in, tx_in_receiver) = channel();
        let (peer_in, peer_in_receiver) = channel();

        self.peer_channel = Some(Arc::new(Mutex::new(peer_in)));
        self.tx_channel = Some(Arc::new(Mutex::new(tx_in)));

        let mut local_self = self.clone();

        thread::spawn(move || {
            let hg = Arc::new(RwLock::new(Hashgraph::new(
                local_self.peers.clone(),
                Arc::new(Mutex::new(tx_out)),
            )));

            let self_peer = Peer::new(local_self.config.listen_addr, local_self.key.get_pub());

            local_self
                .peers
                .write()
                .unwrap()
                .add_self(self_peer.clone());

            let server = HgRpc::Duplex::listen(&local_self.config.listen_addr.to_string());

            {
                let mut guard = server.context.lock().unwrap();
                (*guard).node = Arc::new(RwLock::new(local_self.clone()));
                (*guard).hg = hg.clone();
                (*guard).peers = local_self.peers.clone();
            }

            // Add self root
            hg.write().unwrap().insert_event(Event::new(
                0,
                local_self.peers.read().unwrap().self_id,
                0,
                0,
                vec![],
                vec![],
            ));

            //
            let hg2 = hg.clone();
            thread::spawn(move || loop {
                let tx = tx_in_receiver.recv();

                hg2.write().unwrap().add_self_event(tx.unwrap(), vec![]);
            });

            let hg3 = hg.clone();
            thread::spawn(move || loop {
                let tx = peer_in_receiver.recv();

                hg3.write()
                    .unwrap()
                    .add_self_event(vec![], vec![tx.unwrap()]);
            });

            if let Some(addr) = local_self.config.connect_addr {
                let mut client = HgRpc::Duplex::connect(&addr.to_string());

                let peers = client.ask_join(self_peer).unwrap().unwrap();

                local_self.peers.write().unwrap().merge(peers);
            }

            thread::spawn(move || local_self.gossip(hg));
        });

        tx_out_receiver
    }

    pub fn peer_join(&mut self, peer: Peer) {
        self.peer_channel.clone().map(|mutex| {
            mutex
                .lock()
                .unwrap()
                .send(PeerTx::new(PeerTxType::Join, peer))
                .unwrap();

            mutex
        });
    }

    pub fn add_tx(&mut self, tx: Vec<u8>) {
        self.tx_channel.clone().map(|mutex| {
            mutex.lock().unwrap().send(tx).unwrap();

            mutex
        });
    }

    pub fn close(&self) {
        HgRpc::Duplex::close();
    }

    pub fn gossip(&mut self, _hg: Arc<RwLock<Hashgraph>>) {
        loop {
            thread::sleep(time::Duration::from_millis(1000));

            let peer = match self.peers.read().unwrap().clone().get_random() {
                Some(p) => p,
                None => continue,
            };

            let self_id = self.peers.read().unwrap().self_id;

            let hg = _hg.clone();

            thread::spawn(move || {
                let now = SystemTime::now();

                let mut client = HgRpc::Duplex::connect(&peer.address.to_string());

                // let events_diff = {
                //     let mut hg2 = hg.write().unwrap();

                //     let known = hg2.events.known_events();

                //     let pull_res = client.pull(known);

                //     if let Err(err) = pull_res {
                //         error!("{:?}", err);

                //         continue;
                //     }

                //     let events = pull_res.unwrap().unwrap();

                //     trace!("Events from pull {:?}", events.clone());

                //     let self_id = self.peers.read().unwrap().self_id;

                //     let events_diff = hg2.merge_events(self_id, peer.id, events);

                //     events_diff
                // };

                let known = hg.read().unwrap().events.known_events();

                let pull_res = client.pull(known);

                if let Err(err) = pull_res {
                    error!("{:?}", err);

                    return;
                }

                let events = pull_res.unwrap().unwrap();

                trace!("Events from pull {:?}", events.clone());

                let events_diff = hg.write().unwrap().merge_events(self_id, peer.id, events);

                trace!("Events to push {:?}", events_diff.clone());

                client.push(events_diff);

                debug!("Gossip Time: {:?}", now.elapsed());
            });
        }
    }
}
