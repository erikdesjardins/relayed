use std::io::{self, ErrorKind::*};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use future::poll;
use magic;
use tcp::Conjoin;

pub fn run(public: &SocketAddr, gateway: &SocketAddr) -> Result<(), io::Error> {
    info!("Binding to public {}", public);
    let public_connections = TcpListener::bind(public)?.incoming();
    info!("Binding to gateway {}", public);
    let gateway_connections = TcpListener::bind(gateway)?.incoming();

    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            let magic_byte = poll(gateway, magic::read_byte);

            let timeout = Delay::new(Instant::now() + Duration::from_secs(1)).then(|r| match r {
                Ok(()) => Err(io::Error::from(TimedOut)),
                Err(e) => Err(io::Error::new(Other, e)),
            });

            magic_byte.select(timeout).then(|r| match r {
                Ok(((gateway, _), _)) => {
                    info!("Client handshake succeeded");
                    Ok(Some(gateway))
                }
                Err((e, _)) => {
                    info!("Client handshake failed: {}", e);
                    Ok(None)
                }
            })
        }).filter_map(|x| x);

    let active = Arc::new(AtomicUsize::new(0));

    let server = public_connections
        .zip(gateway_connections)
        .for_each(move |(public, gateway)| {
            info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
            let active = active.clone();
            Ok(spawn(
                // write to notify client that this connection is in use (even if the public client doesn't send anything)
                poll(gateway, magic::write_byte)
                    .and_then(move |(gateway, _)| Conjoin::new(public, gateway))
                    .then(move |r| {
                        let active = active.fetch_sub(1, SeqCst) - 1;
                        Ok(match r {
                            Ok((down, up)) => info!("Closing ({} active): {}/{}", active, down, up),
                            Err(e) => info!("Closing ({} active): {}", active, e),
                        })
                    }),
            ))
        });

    Runtime::new()?.block_on(server)
}
