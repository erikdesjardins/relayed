use std::io::{self, ErrorKind::*};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use backoff::Backoff;
use future::first_ok;
use magic;
use stream;
use tcp::Conjoin;

pub fn run(gateway: &[SocketAddr], private: &[SocketAddr], retry: bool) -> Result<(), io::Error> {
    let backoff = Backoff::new(1..=64);

    let active = Arc::new(AtomicUsize::new(0));

    let server = stream::repeat_with(|| {
        future::ok(())
            .and_then(|()| {
                info!("Connecting to gateway");
                first_ok(gateway, TcpStream::connect, || Err(AddrNotAvailable.into()))
            }).and_then(|gateway| {
                info!("Sending handshake");
                magic::write_to(gateway)
            }).and_then(|gateway| {
                info!("Waiting for handshake response");
                magic::read_from(gateway)
            }).and_then(|gateway| {
                info!("Connecting to private");
                first_ok(private, TcpStream::connect, || Err(AddrNotAvailable.into()))
                    .map(move |private| (gateway, private))
            }).and_then(|(gateway, private)| {
                info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
                let active = active.clone();
                Ok(spawn(Conjoin::new(gateway, private).then(move |r| {
                    let active = active.fetch_sub(1, SeqCst) - 1;
                    Ok(match r {
                        Ok((down, up)) => info!("Closing ({} active): {}/{}", active, down, up),
                        Err(e) => info!("Closing ({} active): {}", active, e),
                    })
                })))
            }).then(|r| match r {
                Ok(()) => {
                    backoff.reset();
                    future::Either::A(future::ok(()))
                }
                Err(ref e) if retry => {
                    error!("{}", e);
                    let seconds = backoff.get();
                    warn!("Retrying in {} seconds", seconds);
                    future::Either::B(
                        Delay::new(Instant::now() + Duration::from_secs(seconds as u64))
                            .map_err(|e| io::Error::new(Other, e)),
                    )
                }
                Err(e) => future::Either::A(future::err(e)),
            })
    }).for_each(Ok);

    Runtime::new()?.block_on(server)
}
