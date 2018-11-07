use std::fmt::{Display, Formatter, Result};
use std::net::SocketAddr;

use super::key::Key;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peer {
    pub id: u64,
    pub address: SocketAddr,
    pub pub_key: Vec<u8>,
}

impl Peer {
    pub fn new(address: SocketAddr, pub_key: Vec<u8>) -> Peer {
        Peer {
            id: Key::pub_to_int(pub_key.clone()),
            address,
            pub_key,
        }
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "Id: {}, Address: {}", self.id, self.address)
    }
}
