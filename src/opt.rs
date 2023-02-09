use clap::{ArgAction, Parser, Subcommand};
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

#[derive(Parser, Debug)]
#[clap(version, about)]
pub struct Options {
    /// Logging verbosity (-v info, -vv debug, -vvv trace)
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub mode: Mode,
}

#[derive(Subcommand, Debug)]
pub enum Mode {
    /// Run the server half on a public machine
    Server {
        /// Socket address to receive gateway connections from client
        gateway: SocketAddr,

        /// Socket address to receive public traffic on
        public: SocketAddr,
    },
    /// Run the client half on a private machine
    Client {
        /// Address of server's gateway
        #[arg(value_parser = socket_addrs)]
        gateway: V<SocketAddr>,

        /// Address to relay public traffic to
        #[arg(value_parser = socket_addrs)]
        private: V<SocketAddr>,
    },
}

/// Alias to avoid clap special-casing `Vec`
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
