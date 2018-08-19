use std::fmt::{self, Display};
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Debug)]
pub enum Mode {
    Server,
    Client,
}

#[derive(Debug)]
pub struct InvalidMode;

impl Display for InvalidMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Mode was not `server` or `client`")
    }
}

impl FromStr for Mode {
    type Err = InvalidMode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "server" => Ok(Mode::Server),
            "client" => Ok(Mode::Client),
            _ => Err(InvalidMode),
        }
    }
}

/// Proxy traffic to a machine behind a dynamic IP/firewall.
/// Sockets can be IPv4 (`0.0.0.0:80`) or IPv6 (`[1:2:3:4::]:80`).
///
/// EXAMPLE: `server 0.0.0.0:8080 0.0.0.0:3000` `client 1.2.3.4:3000 127.0.0.1:80`
#[derive(StructOpt, Debug)]
pub struct Options {
    /// Logging verbosity (-v info, -vv debug, -vvv trace)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    pub verbose: u8,

    /// Retry when connection fails (exponential backoff)
    #[structopt(short = "r", long = "retry")]
    pub retry: bool,

    /// `server` or `client`
    pub mode: Mode,

    /// For servers, the socket to proxy from; for clients, the server's gateway socket
    pub from: SocketAddr,

    /// For servers, the gateway socket;       for clients, the socket to proxy to
    pub to: SocketAddr,
}
