# hashgraph
Hashgraph consensus in rust

This is a work in progress.

## Usage

```rust
use hashgraph::{Key, Node, NodeConfig};

fn main() {
    let node = Node::new(Key::generate_new(), NodeConfig::default());

    let (tx_in, tx_out) = node.run();

    tx_in.send("Some transaction".to_string().into_bytes()).unwrap();

    loop {
        let res = tx_out.recv();

        println!("{:?}", res);
    }
}
```
