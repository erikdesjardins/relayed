use std::net::SocketAddr;

use failure::Error;

use util::Backoff;

pub fn run(gateway: &SocketAddr, private: &SocketAddr, retry: bool) -> Result<(), Error> {
    let backoff = Backoff::new(1..=64);

    Ok(())
}
