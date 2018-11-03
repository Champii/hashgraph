use clap::{App, Arg};
use std::net::SocketAddr;

use super::hashgraph::NodeConfig;

pub fn to_socket_addr(s: &str) -> SocketAddr {
  match s.parse::<SocketAddr>() {
    Ok(addr) => addr,
    Err(e) => {
      panic!("Invalid address: {}, {}", s, e);
    }
  }
}

pub fn parse_config() -> NodeConfig {
  let matches = App::new("Rust-Hashgraph")
    .version("1.0")
    .author("Champii <contact@champii.io>")
    .about("Simple hashgraph in Rust")
    .arg(
      Arg::with_name("listen")
        .short("l")
        .long("listen")
        .value_name("IpAddr")
        .help("Listening address (127.0.0.1:3000)")
        .takes_value(true),
    )
    // .arg(
    //   Arg::with_name("connect")
    //     .short("c")
    //     .long("connect")
    //     .value_name("IpAddr")
    //     .help("Connect to bootstrap node")
    //     .takes_value(true),
    // )
    .arg(
      Arg::with_name("verbose")
        .short("v")
        .long("verbose")
        .value_name("Level")
        .help("Verbose level (between 0-5, default 2)")
        .takes_value(true),
    )
    .get_matches();

  // let connect_addr_str = matches.value_of("connect").unwrap_or("");
  // let connect_addr = if connect_addr_str == "" {
  //   None
  // } else {
  //   Some(to_socket_addr(connect_addr_str))
  // };

  let listen_addr_str = matches.value_of("listen").unwrap_or("127.0.0.1:3000");
  let listen_addr = to_socket_addr(listen_addr_str);

  let verbose = matches
    .value_of("verbose")
    .unwrap_or("0")
    .parse::<u8>()
    .unwrap();

  NodeConfig {
    listen_addr,
    // connect_addr,
    verbose,
  }
}
