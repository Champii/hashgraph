#![feature(
  async_await,
  await_macro,
  pin,
  arbitrary_self_types,
  futures_api
)]

#[macro_use]
extern crate log;
extern crate clap;
extern crate hashgraph;

mod args;

use hashgraph::logger;
use hashgraph::Key;
use hashgraph::Node;

fn main() {
  let config = args::parse_config();

  logger::init_logger(config.verbose);

  let key = Key::new_generate().unwrap();

  let mut node = Node::new(key, config);

  let (tx_in, tx_out) = node.run();

  loop {
    let res = tx_out.recv();

    println!("RESULT {:?}", res);
  }
}
