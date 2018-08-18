use std::net::SocketAddr;

use failure::Error;
use tokio;
use tokio::net::TcpStream;
use tokio::prelude::*;

use conjoin;
use util::Backoff;

pub fn run(gateway: &SocketAddr, private: &SocketAddr, retry: bool) -> Result<(), Error> {
    let backoff = Backoff::new(1..=64);

    let one_copy = TcpStream::connect(gateway)
        .join(TcpStream::connect(private))
        .and_then(|(gateway, private)| {
            match (gateway.peer_addr(), private.peer_addr()) {
                (Ok(p), Ok(g)) => info!("Copying from {} to {}", p, g),
                (Err(e), _) | (_, Err(e)) => warn!("Error getting peer address: {}", e),
            }
            conjoin::tcp(gateway, private)
        })
        .and_then(|(bytes_out, bytes_in)| {
            info!("{} bytes out, {} bytes in", bytes_out, bytes_in);
            Ok(())
        })
        .map_err(|e| warn!("Error while copying: {}", e));

    tokio::run(one_copy);

    Ok(())
}
