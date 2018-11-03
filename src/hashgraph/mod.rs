#![feature(duration_as_u128)]

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate rsrpc;
#[macro_use]
pub extern crate lazy_static;
#[macro_use]
extern crate log;

extern crate bincode;
extern crate env_logger;
extern crate rand;
extern crate ring;
extern crate serde;
extern crate serde_bytes;
extern crate untrusted;

use std::net::SocketAddr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{thread, time};

mod event;
mod events;
mod hashgraph;
mod key;
pub mod logger;
mod peer;
mod peers;
mod round;
mod rpc;

use event::Event;
use hashgraph::Hashgraph;
pub use key::Key;
pub use peer::Peer;
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
    pub peers: Arc<Mutex<Peers>>,
    // hg: Arc<Mutex<Hashgraph>>,
    // pub tx_channel: Receiver<Vec<u8>>,
}

impl Node {
    pub fn new(key: Key, config: NodeConfig) -> Node {
        let peers = Arc::new(Mutex::new(Peers::new()));

        Node {
            key,
            config,
            peers: peers.clone(),
            // hg: Arc::new(Mutex::new(Hashgraph::new(peers, sender))),
            // tx_channel: receiver,
        }
    }

    pub fn run(&mut self) -> Receiver<Vec<u8>> {
        let (sender, receiver) = channel();

        let mut local_self = self.clone();

        thread::spawn(move || {
            let hg = Arc::new(Mutex::new(Hashgraph::new(local_self.peers.clone(), sender)));

            local_self.peers.lock().unwrap().add_self(Peer::new(
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
            hg.lock().unwrap().insert_event(Event::new(
                0,
                local_self.peers.lock().unwrap().self_id,
                0,
                0,
                vec![],
            ));

            //
            local_self.gossip(hg);
        });

        receiver
    }

    pub fn close(&self) {
        HgRpc::Duplex::close();
    }

    pub fn gossip(&mut self, hg: Arc<Mutex<Hashgraph>>) {
        loop {
            thread::sleep(time::Duration::from_millis(1000));

            let peer;

            {
                peer = match self.peers.lock().unwrap().clone().get_random() {
                    Some(p) => p,
                    None => continue,
                };
            }

            let mut client = HgRpc::Duplex::connect(&peer.address.to_string());

            let known = hg.lock().unwrap().events.known_events();

            client
                .pull(known)
                .and_then(|events| {
                    let res = events.unwrap();
                    trace!("Events from pull {:?}", res.clone());

                    let events_diff = hg.lock().unwrap().merge_events(
                        self.peers.lock().unwrap().self_id,
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
        }
    }
}
