use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::timer::Delay;

use backoff::Backoff;
use stream;
use tcp::Conjoin;

pub fn run(gateway: SocketAddr, private: SocketAddr, retry: bool) {
    let backoff = Arc::new(Backoff::new(1..=64));

    let server = stream::repeat_with(move || {
        let backoff = backoff.clone();
        TcpStream::connect(&gateway)
            .join(TcpStream::connect(&private))
            .and_then(|(gateway, private)| {
                match (gateway.peer_addr(), private.peer_addr()) {
                    (Ok(p), Ok(g)) => info!("Copying from {} to {}", p, g),
                    (Err(e), _) | (_, Err(e)) => warn!("Error getting peer address: {}", e),
                }
                Conjoin::new(gateway, private).then(|r| {
                    match r {
                        Ok((bytes_out, bytes_in)) => {
                            info!("{} bytes out, {} bytes in", bytes_out, bytes_in)
                        }
                        Err(e) => warn!("Failed to copy: {}", e),
                    }
                    Ok(())
                })
            })
            .then(move |r| match r {
                Ok(()) => {
                    backoff.reset();
                    future::Either::A(Ok(()).into_future())
                }
                Err(e) => {
                    error!("{}", e);
                    if retry {
                        let seconds = backoff.get();
                        info!("Retrying in {}s...", seconds);
                        future::Either::B(
                            Delay::new(Instant::now() + Duration::from_secs(seconds as u64))
                                .map_err(|e| panic!("Error setting retry delay: {}", e)),
                        )
                    } else {
                        future::Either::A(Err(()).into_future())
                    }
                }
            })
    }).for_each(|()| Ok(()));

    tokio::run(server);
}
