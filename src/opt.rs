use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::ops::Deref;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Options {
    /// Logging verbosity (-v info, -vv debug, -vvv trace)
    #[structopt(
        short = "v",
        long = "verbose",
        parse(from_occurrences),
        raw(global = "true")
    )]
    pub verbose: u8,

    #[structopt(subcommand)]
    pub mode: Mode,
}

#[derive(StructOpt, Debug)]
pub enum Mode {
    /// Run the server half on a public machine
    #[structopt(name = "server")]
    Server {
        /// Port to receive gateway connections from client
        gateway: u16,

        /// Port to receive public traffic on
        public: u16,
    },
    /// Run the client half on a private machine
    #[structopt(name = "client")]
    Client {
        /// Address of server's gateway
        #[structopt(parse(try_from_str = "socket_addrs"))]
        gateway: A<Vec<SocketAddr>>,

        /// Address to relay public traffic to
        #[structopt(parse(try_from_str = "socket_addrs"))]
        private: A<Vec<SocketAddr>>,

        /// Retry when connection fails (exponential backoff)
        #[structopt(short = "r", long = "retry")]
        retry: bool,
    },
}

/// Trivial wrapper to avoid structopt special-casing `Vec`
#[derive(Debug)]
pub struct A<T>(T);

impl<T> Deref for A<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn socket_addrs(arg: &str) -> Result<A<Vec<SocketAddr>>, io::Error> {
    let addrs = arg.to_socket_addrs()?.collect::<Vec<_>>();
    match addrs.len() {
        0 => Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "Resolved to zero addresses",
        )),
        _ => Ok(A(addrs)),
    }
}
