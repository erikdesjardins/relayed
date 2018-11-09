use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::{Duration, Instant};

use log::{error, info, warn};
use tokio::executor::current_thread::spawn;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use backoff::Backoff;
use config::BACKOFF_SECS;
use err;
use future::first_ok;
use magic;
use stream;
use tcp;

pub fn run(
    gateway_addrs: &[SocketAddr],
    private_addrs: &[SocketAddr],
    retry: bool,
) -> Result<(), io::Error> {
    let mut runtime = Runtime::new()?;

    let backoff = Backoff::new(BACKOFF_SECS);

    let active = Rc::new(AtomicUsize::new(0));

    let client = stream::repeat_with(|| {
        future::ok(())
            .and_then(|()| {
                info!("Connecting to gateway");
                first_ok(gateway_addrs.iter().map(TcpStream::connect))
            }).and_then(|gateway| {
                info!("Sending early handshake");
                magic::write_to(gateway)
            }).and_then(|gateway| {
                info!("Waiting for late handshake");
                magic::read_from(gateway)
            }).and_then(|gateway| {
                info!("Sending late handshake response");
                magic::write_to(gateway)
            }).and_then(|gateway| {
                info!("Connecting to private");
                first_ok(private_addrs.iter().map(TcpStream::connect))
                    .map(move |private| (gateway, private))
            }).and_then(|(gateway, private)| {
                info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
                let active = active.clone();
                Ok(spawn(tcp::conjoin(gateway, private).then(move |r| {
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
                            .map_err(err::to_io()),
                    )
                }
                Err(e) => future::Either::A(future::err(e)),
            })
    }).for_each(Ok);

    runtime.block_on(client)
}
