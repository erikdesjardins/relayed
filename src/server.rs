use std::io::{self, ErrorKind::*};
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::{Duration, Instant};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use magic;
use tcp;

pub fn run(public: &SocketAddr, gateway: &SocketAddr) -> Result<(), io::Error> {
    info!("Binding to public {}", public);
    let public_connections = TcpListener::bind(public)?.incoming();
    info!("Binding to gateway {}", gateway);
    let gateway_connections = TcpListener::bind(gateway)?.incoming();

    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            let magic_byte = magic::read_from(gateway);

            let timeout = Delay::new(Instant::now() + Duration::from_secs(1)).then(|r| match r {
                Ok(()) => Err(io::Error::from(TimedOut)),
                Err(e) => Err(io::Error::new(Other, e)),
            });

            magic_byte.select(timeout).then(|r| match r {
                Ok((gateway, _)) => {
                    info!("Client handshake succeeded");
                    Ok(Some(gateway))
                }
                Err((e, _)) => {
                    info!("Client handshake failed: {}", e);
                    Ok(None)
                }
            })
        }).filter_map(|x| x);

    let active = Rc::new(AtomicUsize::new(0));

    let server = public_connections
        .zip(gateway_connections)
        .for_each(move |(public, gateway)| {
            info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
            let active = active.clone();
            Ok(spawn(
                // write to notify client that this connection is in use (even if the public side doesn't send anything)
                magic::write_to(gateway)
                    .and_then(move |gateway| tcp::conjoin(public, gateway))
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
