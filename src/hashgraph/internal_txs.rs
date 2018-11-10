use super::peer::Peer;

#[derive(Hash, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerTxType {
    Join,
    Leave,
}

#[derive(Hash, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerTx {
    pub tx_type: PeerTxType,
    pub peer: Peer,
}

impl PeerTx {
    pub fn new(tx_type: PeerTxType, peer: Peer) -> PeerTx {
        PeerTx { tx_type, peer }
    }
}
