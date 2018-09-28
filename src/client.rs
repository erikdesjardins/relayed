use std::io::{self, ErrorKind::*};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use backoff::Backoff;
use future::{first_ok, poll};
use magic;
use stream;
use tcp::Conjoin;

pub fn run(gateway: &[SocketAddr], private: &[SocketAddr], retry: bool) -> Result<(), io::Error> {
    let backoff = Backoff::new(1..=64);

    let server = stream::repeat_with(|| {
        future::ok(())
            .and_then(|()| {
                info!("Connecting to gateway");
                first_ok(gateway, TcpStream::connect, || Err(AddrNotAvailable.into()))
            }).and_then(|gateway| {
                info!("Sending handshake");
                poll(gateway, magic::write_byte)
            }).and_then(|(gateway, _)| {
                info!("Waiting for handshake response");
                poll(gateway, magic::read_byte)
            }).and_then(|(gateway, _)| {
                info!("Connecting to private");
                first_ok(private, TcpStream::connect, || Err(AddrNotAvailable.into()))
                    .map(move |private| (gateway, private))
            }).and_then(|(gateway, private)| {
                info!("Spawning connection handler");
                Ok(spawn(Conjoin::new(gateway, private).then(|r| {
                    Ok(match r {
                        Ok((down, up)) => info!("Closing connection: {} down, {} up", down, up),
                        Err(e) => info!("Closing connection: {}", e),
                    })
                })))
            }).then(|r| match r {
                Ok(()) => {
                    backoff.reset();
                    future::Either::A(future::ok(()))
                }
                Err(ref e) if retry => {
                    error!("Opening connection failed: {}", e);
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
