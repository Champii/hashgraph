# hashgraph
Hashgraph consensus in rust

This is a work in progress.

This consensus protocol is liscensed by [Swirlds](https://www.swirlds.com/) for 
US citizens, you might want to check how to aquire that liscence.

I intend to modify that algorythm to match the one from [MaidSafe](https://maidsafe.net/)
that is free and open source: [PARSEC](https://medium.com/safenetwork/parsec-a-paradigm-shift-for-asynchronous-and-permissionless-consensus-e312d721f9d8)

## Usage

```rust
use hashgraph::{Key, Node, NodeConfig};

fn main() {
    let node = Node::new(Key::generate_new(), NodeConfig::default());

    let (tx_in, tx_out) = node.run();

    tx_in.send("Some transaction".to_string().into_bytes()).unwrap();

    loop {
        let res = tx_out.recv();

        println!("{:?}", String::from_utf8(res.unwrap()).unwrap());
    }
}
```

## Features
- [x] Gossip
- [x] Event merge
- [ ] Hashgraph consensus
    - [x] Ancestors
    - [x] Self Ancestors
    - [x] Strongly see
    - [x] Witness
    - [x] Round
    - [x] Round Received
    - [x] Virtual voting
    - [x] Famous Witness
    - [x] Consensus Timestamp
    - [ ] Break ties on same Consensus Timestamp (Median timestamp)
    - [ ] Break ties on same Consensus Timestamp (Signature XOR)
    - [x] Transaction submit
    - [x] Consensus Transaction output
    - [x] Dynamic participants
        - [x] Peer add
        - [x] Peer Remove
- [x] Caching for performances
- [x] Sync
- [ ] Event Signature
- [ ] Implement the PARSEC specification instead of the Swirlds one
- [ ] Transaction validation
- [ ] Bad peer removal or punishement
- [ ] Node test
