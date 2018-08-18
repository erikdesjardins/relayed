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
#[derive(StructOpt, Debug)]
pub struct Options {
    /// Logging verbosity (-v info, -vv debug, -vvv trace)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    pub verbose: u8,

    /// `server` or `client` mode
    pub mode: Mode,

    /// For servers, the socket to proxy from; for clients, the server to connect to
    pub from: SocketAddr,

    /// For servers, the internal socket for the client; for clients, the socket to proxy to
    pub to: SocketAddr,
}
