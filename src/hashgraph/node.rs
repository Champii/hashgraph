use rsrpc::TcpTransport;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use std::{thread, time};

use super::trace_time;
use event::Event;
use hashgraph::Hashgraph;
use internal_txs::{PeerTx, PeerTxType};
use key::Key;
use peer::Peer;
use peers::Peers;
use round::Round;
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

        let syncing = Arc::new(RwLock::new(true));
        let syncing2 = syncing.clone();

        thread::spawn(move || {
            let self_peer = Peer::new(local_self.config.listen_addr, local_self.key.get_pub());

            local_self.peers.write().unwrap().self_id = self_peer.id;

            let hg = Arc::new(RwLock::new(Hashgraph::new(Arc::new(Mutex::new(tx_out)))));

            let hg2 = hg.clone();
            thread::spawn(move || loop {
                if *syncing2.read().unwrap() {
                    continue;
                }

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
                *syncing.write().unwrap() = true;

                Node::sync(hg.clone(), addr, self_peer);
            } else {
                local_self
                    .peers
                    .write()
                    .unwrap()
                    .add_self(self_peer.clone());

                hg.write()
                    .unwrap()
                    .bootstrap(local_self.peers.read().unwrap().clone());

                // bootstrap node, we add the peer_tx on the root
                hg.write().unwrap().insert_event(Event::new(
                    0,
                    self_peer.id,
                    0,
                    0,
                    vec![],
                    vec![PeerTx::new_join(self_peer.clone())],
                ));
            }

            *syncing.write().unwrap() = false;

            let server = HgRpc::listen_tcp(&local_self.config.listen_addr.to_string());

            {
                let mut guard = server.context.lock().unwrap();
                (*guard).node = Arc::new(RwLock::new(local_self.clone()));
                (*guard).hg = hg.clone();
                (*guard).peers = local_self.peers.clone();
            }

            local_self.gossip(hg);
        });

        tx_out_receiver
    }

    fn sync(hg: Arc<RwLock<Hashgraph>>, addr: SocketAddr, self_peer: Peer) {
        let mut client = HgRpc::connect_tcp(&addr.to_string()).unwrap();

        client.ask_join(self_peer.clone()).unwrap().unwrap();

        info!("Syncing...");

        let mut frame;

        loop {
            let res = client.fast_sync(self_peer.id);

            if let Err(err) = res {
                error!("{:?}", err);

                client.close();

                return;
            }

            frame = res.unwrap().unwrap();

            if frame.events.len() == 0 {
                debug!("Waiting for acceptation");

                thread::sleep(time::Duration::from_millis(1000));
            } else {
                break;
            }
        }

        trace!("Sync: rounds {}", frame.events.len());

        trace_time!("Sync");

        let now = SystemTime::now();

        let mut hg = hg.write().unwrap();

        let mut nb_events = 0;
        let mut nb_peers = 0;

        for (round_id, round_events) in frame.clone().events {
            hg.rounds.entry(round_id).or_insert_with(|| {
                let mut round = Round::new(round_id);

                round.peers = round_events.0.clone();

                round.peers.self_id = self_peer.id;

                nb_peers = round.peers.len();

                round
            });

            for (_, events) in round_events.1 {
                for (_, event) in events {
                    hg.insert_event(event.clone());

                    nb_events += 1;
                }
            }
        }

        client.close();

        info!("Synced: Events {}, Peers {}", nb_events, nb_peers);

        hg.insert_event(Event::new(
            0,
            self_peer.id,
            0,
            0,
            vec![self_peer.id.to_string().into_bytes()],
            vec![],
        ));
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
        let mut clients: HashMap<u64, HgRpc::Client<rsrpc::TcpTransport>> = HashMap::new();

        loop {
            let peer = match _hg.read().unwrap().get_last_decided_peers().get_random() {
                Some(p) => p,
                None => {
                    thread::sleep(time::Duration::from_millis(1000));

                    continue;
                }
            };

            trace_time!("Gossip");

            let mut client = {
                let client = clients.get(&peer.id);

                if client.is_some() {
                    client.unwrap().clone()
                } else {
                    let c = HgRpc::connect_tcp(&peer.address.to_string());

                    if let Err(e) = c {
                        debug!("Error connect: {:?} {}", peer.address, e);
                        // send leave tx

                        continue;
                    }

                    let c = c.unwrap();

                    c
                }
            };

            clients.insert(peer.id, client.clone());

            let self_id = _hg.read().unwrap().get_last_decided_peers().self_id;

            let hg = _hg.clone();

            let now = SystemTime::now();

            let known = hg.read().unwrap().events.known_events();

            let pull_res = client.pull(known);

            if let Err(err) = pull_res {
                error!("{:?}", err);

                client.close();

                clients.remove(&peer.id);

                continue;
            }

            let events = pull_res.unwrap().unwrap();

            trace!(
                "Events from pull {:?}",
                events.diff.iter().fold(0, |c, v| c + v.1.len())
            );

            let events_diff = hg.write().unwrap().merge_events(self_id, peer.id, events);

            if events_diff.is_err() {
                continue;
            }

            let events_diff = events_diff.unwrap();

            trace!(
                "Events to push {:?}",
                events_diff.diff.iter().fold(0, |c, v| c + v.1.len())
            );

            client.push(events_diff);
        }
    }
}
