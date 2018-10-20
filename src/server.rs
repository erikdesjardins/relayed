use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;

use config::{HANDSHAKE_TIMEOUT, TRANSFER_TIMEOUT};
use future::FutureExt;
use magic;
use tcp;

pub fn run(public: &SocketAddr, gateway: &SocketAddr) -> Result<(), io::Error> {
    info!("Binding to public {}", public);
    let public_connections = TcpListener::bind(public)?.incoming();
    info!("Binding to gateway {}", gateway);
    let gateway_connections = TcpListener::bind(gateway)?.incoming();

    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            magic::read_from(gateway)
                .timeout_after_inactivity(HANDSHAKE_TIMEOUT)
                .then(|r| match r {
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
                    .timeout_after_inactivity(TRANSFER_TIMEOUT)
                    .then(move |r| {
                        let active = active.fetch_sub(1, SeqCst) - 1;
                        Ok(match r {
                            Ok(((down, up), _)) => info!("Closing ({} active): {}/{}", active, down, up),
                            Err((e, _)) => info!("Closing ({} active): {}", active, e),
                        })
                    }),
            ))
        });

    Runtime::new()?.block_on(server)
}
