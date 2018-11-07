use std::net::SocketAddr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::{thread, time};

use event::Event;
use hashgraph::Hashgraph;
use key::Key;
use peer::Peer;
use peers::Peers;
use rpc::HgRpc;

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub verbose: u8,
    pub listen_addr: SocketAddr,
    // pub connect_addr: Option<SocketAddr>,
}

#[derive(Clone)]
pub struct Node {
    key: Key,
    pub config: NodeConfig,
    pub peers: Arc<RwLock<Peers>>,
    // hg: Arc<Mutex<Hashgraph>>,
    // pub tx_channel: Receiver<Vec<u8>>,
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
        }
    }

    pub fn run(&mut self) -> (Sender<Vec<u8>>, Receiver<Vec<u8>>) {
        let (tx_out, tx_out_receiver) = channel();
        let (tx_in, tx_in_receiver) = channel();

        let mut local_self = self.clone();

        thread::spawn(move || {
            let hg = Arc::new(RwLock::new(Hashgraph::new(
                local_self.peers.clone(),
                Arc::new(Mutex::new(tx_out)),
            )));

            local_self.peers.write().unwrap().add_self(Peer::new(
                local_self.config.listen_addr,
                local_self.key.get_pub(),
            ));

            let server = HgRpc::Duplex::listen(&local_self.config.listen_addr.to_string());

            {
                let mut guard = server.context.lock().unwrap();
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
            ));

            //
            let hg2 = hg.clone();
            thread::spawn(move || loop {
                let tx = tx_in_receiver.recv();

                hg2.write().unwrap().add_transaction(tx.unwrap());
            });

            local_self.gossip(hg);
        });

        (tx_in, tx_out_receiver)
    }

    pub fn close(&self) {
        HgRpc::Duplex::close();
    }

    pub fn gossip(&mut self, hg: Arc<RwLock<Hashgraph>>) {
        loop {
            thread::sleep(time::Duration::from_millis(1000));

            let peer = match self.peers.read().unwrap().clone().get_random() {
                Some(p) => p,
                None => continue,
            };

            let mut client = HgRpc::Duplex::connect(&peer.address.to_string());

            let known = hg.read().unwrap().events.known_events();

            client
                .pull(known)
                .and_then(|events| {
                    let res = events.unwrap();
                    trace!("Events from pull {:?}", res.clone());

                    let events_diff = hg.write().unwrap().merge_events(
                        self.peers.read().unwrap().self_id,
                        peer.id,
                        res,
                    );

                    trace!("Events to push {:?}", events_diff.clone());

                    let res = client.push(events_diff);

                    Ok(res)
                })
                .or_else(|err| {
                    error!("{:?}", err);

                    Err(err)
                });
            // .unwrap() // !!!!
            // .unwrap() // !!!!
            // .unwrap(); // !!!!
        }
    }
}
