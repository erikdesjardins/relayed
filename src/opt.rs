use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(about)]
pub struct Options {
    /// Logging verbosity (-v info, -vv debug, -vvv trace)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences), global = true)]
    pub verbose: u8,

    #[structopt(subcommand)]
    pub mode: Mode,
}

#[derive(StructOpt, Debug)]
pub enum Mode {
    /// Run the server half on a public machine
    #[structopt(name = "server")]
    Server {
        /// Socket address to receive gateway connections from client
        gateway: SocketAddr,

        /// Socket address to receive public traffic on
        public: SocketAddr,
    },
    /// Run the client half on a private machine
    #[structopt(name = "client")]
    Client {
        /// Address of server's gateway
        #[structopt(parse(try_from_str = socket_addrs))]
        gateway: V<SocketAddr>,

        /// Address to relay public traffic to
        #[structopt(parse(try_from_str = socket_addrs))]
        private: V<SocketAddr>,

        /// Retry when connection fails (exponential backoff)
        #[structopt(short = "r", long = "retry")]
        retry: bool,
    },
}

/// Alias to avoid structopt special-casing `Vec`
type V<T> = Vec<T>;

fn socket_addrs(arg: &str) -> Result<Vec<SocketAddr>, io::Error> {
    let addrs = arg.to_socket_addrs()?.collect::<Vec<_>>();
    match addrs.len() {
        0 => Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "Resolved to zero addresses",
        )),
        _ => Ok(addrs),
    }
}
