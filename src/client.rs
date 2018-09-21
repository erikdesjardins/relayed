use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use backoff::Backoff;
use stream;
use tcp::LazyConjoin;

fn test_addrs(addrs: Vec<SocketAddr>, err: &'static str) -> Result<SocketAddr, io::Error> {
    for addr in addrs {
        match TcpStream::connect(&addr).wait() {
            Ok(_) => return Ok(addr),
            Err(e) => warn!("{}: {}", addr, e),
        }
    }
    Err(io::Error::new(io::ErrorKind::Other, err))
}

pub fn run(
    gateway: Vec<SocketAddr>,
    private: Vec<SocketAddr>,
    retry: bool,
) -> Result<(), io::Error> {
    let backoff = Rc::new(Backoff::new(1..=64));

    let gateway = test_addrs(gateway, "Unable to connect to gateway")?;
    let private = test_addrs(private, "Unable to connect to private")?;

    let server = stream::repeat_with(move || {
        let backoff = backoff.clone();
        TcpStream::connect(&gateway)
            .join(TcpStream::connect(&private))
            .and_then(|(gateway, private)| {
                match (gateway.peer_addr(), private.peer_addr()) {
                    (Ok(p), Ok(g)) => info!("Transferring from {} to {}", p, g),
                    (Err(e), _) | (_, Err(e)) => warn!("Error getting peer address: {}", e),
                }
                // once we receive data from either side, spawn a task to handle that connection
                // and open a new connection
                // (i.e. there will always be one connection ready, as long as no client opens a
                //  connection without sending data, which can happen, but for simplicity we don't
                //  handle that)
                LazyConjoin::new(gateway, private).and_then(|conjoin| {
                    let conjoin = conjoin.then(|r| {
                        match r {
                            Ok((bytes_down, bytes_up)) => info!(
                                "Transfer complete: {} bytes down, {} bytes up",
                                bytes_down, bytes_up
                            ),
                            Err(e) => warn!("Transfer cancelled: {}", e),
                        }
                        Ok(())
                    });

                    spawn(conjoin);

                    Ok(())
                })
            }).then(move |r| match r {
                Ok(()) => {
                    backoff.reset();
                    future::Either::A(future::ok(()))
                }
                Err(e) => {
                    if retry {
                        error!("{}", e);
                        let seconds = backoff.get();
                        info!("Retrying in {}s...", seconds);
                        future::Either::B(
                            Delay::new(Instant::now() + Duration::from_secs(seconds as u64))
                                .map_err(|e| panic!("Error setting retry delay: {}", e)),
                        )
                    } else {
                        future::Either::A(future::err(e))
                    }
                }
            })
    }).for_each(|()| Ok(()));

    Runtime::new()?.block_on(server)
}
