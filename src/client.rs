use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::{Duration, Instant};

use futures::future::Either;
use tokio::executor::current_thread::spawn;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use crate::backoff::Backoff;
use crate::config::{BACKOFF_SECS, KEEPALIVE_TIMEOUT};
use crate::err;
use crate::heartbeat;
use crate::magic;
use crate::tcp;

pub fn run(
    gateway_addrs: &[SocketAddr],
    private_addrs: &[SocketAddr],
    retry: bool,
) -> Result<(), io::Error> {
    let mut runtime = Runtime::new()?;

    let backoff = Backoff::new(BACKOFF_SECS);

    let active = Rc::new(AtomicUsize::new(0));

    let client = stream::repeat(())
        .and_then(|()| {
            log::info!("Connecting to gateway");
            future::select_ok(gateway_addrs.iter().map(TcpStream::connect))
                .map(|(gateway, _)| gateway)
        })
        .and_then(|gateway| {
            log::info!("Sending early handshake");
            magic::write_to(gateway)
        })
        .and_then(|gateway| {
            log::info!("Waiting for end of heartbeat");
            heartbeat::read_from(gateway)
        })
        .and_then(|gateway| {
            log::info!("Sending late handshake");
            magic::write_to(gateway)
        })
        .and_then(|gateway| {
            log::info!("Connecting to private");
            future::select_ok(private_addrs.iter().map(TcpStream::connect))
                .map(move |(private, _)| (gateway, private))
        })
        .and_then(|(gateway, private)| {
            gateway.set_keepalive(Some(KEEPALIVE_TIMEOUT))?;
            private.set_keepalive(Some(KEEPALIVE_TIMEOUT))?;
            log::info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
            let active = active.clone();
            Ok(spawn(tcp::conjoin(gateway, private).then(move |r| {
                let active = active.fetch_sub(1, SeqCst) - 1;
                Ok(match r {
                    Ok((down, up)) => log::info!("Closing ({} active): {}/{}", active, down, up),
                    Err(e) => log::info!("Closing ({} active): {}", active, e),
                })
            })))
        })
        .then(|r| match r {
            Ok(()) => {
                backoff.reset();
                Either::A(future::ok(()))
            }
            Err(ref e) if retry => {
                log::error!("{}", e);
                let seconds = backoff.get();
                log::warn!("Retrying in {} seconds", seconds);
                Either::B(
                    Delay::new(Instant::now() + Duration::from_secs(seconds as u64))
                        .map_err(err::to_io()),
                )
            }
            Err(e) => Either::A(future::err(e)),
        })
        .for_each(Ok);

    runtime.block_on(client)
}
