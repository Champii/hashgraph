#![feature(async_await, await_macro, pin, arbitrary_self_types, futures_api)]

extern crate clap;
extern crate hashgraph;
extern crate logger;

mod args;

use hashgraph::Key;
use hashgraph::Node;

fn main() {
  let config = args::parse_config();

  logger::init_logger(config.verbose);

  let key = Key::new_generate().unwrap();

  let mut node = Node::new(key.clone(), config);

  let tx_out = node.run();

  node.add_tx(key.get_pub());

  loop {
    let res = tx_out.recv();

    println!("RESULT {:?}", res);
  }
}
