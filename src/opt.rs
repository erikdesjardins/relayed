use std::io::{self, ErrorKind::*};
use std::net::{SocketAddr, ToSocketAddrs};

/// Trivial wrapper to avoid structopt special-casing `Vec`
#[derive(Debug)]
pub struct A<T>(pub T);

#[derive(StructOpt, Debug)]
#[structopt(
    about = "\
Relay a TCP socket to a machine behind a dynamic IP/firewall.

EXAMPLE: `server 0.0.0.0:8080 0.0.0.0:3000` `client srv.com:3000 localhost:80`
    public traffic -->--/         \\--<-- relay tunnel --<--/         \\-->-- private service
"
)]
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
        /// Port to receive public traffic
        public: u16,

        /// Port to receive gateway connections from client
        gateway: u16,
    },
    /// Run the client half on a private machine
    #[structopt(name = "client")]
    Client {
        /// Address of server's gateway
        #[structopt(parse(try_from_str = "socket_addrs"))]
        gateway: A<Vec<SocketAddr>>,

        /// Address to receive traffic from server
        #[structopt(parse(try_from_str = "socket_addrs"))]
        private: A<Vec<SocketAddr>>,

        /// Retry when connection fails (exponential backoff)
        #[structopt(short = "r", long = "retry")]
        retry: bool,
    },
}

fn socket_addrs(arg: &str) -> Result<A<Vec<SocketAddr>>, io::Error> {
    let addrs = arg.to_socket_addrs()?.collect::<Vec<_>>();
    match addrs.len() {
        0 => Err(io::Error::new(
            AddrNotAvailable,
            "Resolved to zero addresses",
        )),
        _ => Ok(A(addrs)),
    }
}
