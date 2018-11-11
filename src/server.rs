use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};

use futures::sync::mpsc;
use log::{debug, info};
use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;

use config::{HANDSHAKE_TIMEOUT, TRANSFER_TIMEOUT};
use future::FutureExt;
use magic;
use stream::zip_left_then_right;
use tcp;

pub fn run(public_addr: &SocketAddr, gateway_addr: &SocketAddr) -> Result<(), io::Error> {
    let mut runtime = Runtime::new()?;

    info!("Binding to public {}", public_addr);
    let public_connections = TcpListener::bind(public_addr)?.incoming();
    info!("Binding to gateway {}", gateway_addr);
    let gateway_connections = TcpListener::bind(gateway_addr)?.incoming();

    // early handshake: immediately kill unknown connections
    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            future::ok(gateway)
                .and_then(magic::read_from)
                .timeout_after_inactivity(HANDSHAKE_TIMEOUT)
                .then(|r| match r {
                    Ok((gateway, _)) => {
                        debug!("Early handshake succeeded");
                        Ok(Some(gateway))
                    }
                    Err((e, _)) => {
                        debug!("Early handshake failed: {}", e);
                        Ok(None)
                    }
                })
        }).filter_map(|x| x);

    let gateway_connections = mpsc::spawn_unbounded(gateway_connections, &runtime.handle());

    // late handshake: ensure that client hasn't disappeared some time after early handshake
    // and notify client that this connection is going to be used so it should open a new one
    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            future::ok(gateway)
                .and_then(magic::write_to)
                .and_then(magic::read_from)
                .timeout_after_inactivity(HANDSHAKE_TIMEOUT)
                .then(|r| match r {
                    Ok((gateway, _)) => {
                        debug!("Late handshake succeeded");
                        Ok(Some(gateway))
                    }
                    Err((e, _)) => {
                        debug!("Late handshake failed: {}", e);
                        Ok(None)
                    }
                })
        }).filter_map(|x| x);

    let active = Rc::new(AtomicUsize::new(0));

    let server = zip_left_then_right(public_connections, gateway_connections).for_each(
        move |(public, gateway)| {
            info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
            let active = active.clone();
            Ok(spawn(
                tcp::conjoin(public, gateway)
                    .timeout_after_inactivity(TRANSFER_TIMEOUT)
                    .then(move |r| {
                        let active = active.fetch_sub(1, SeqCst) - 1;
                        Ok(match r {
                            Ok(((down, up), _)) => {
                                info!("Closing ({} active): {}/{}", active, down, up)
                            }
                            Err((e, _)) => info!("Closing ({} active): {}", active, e),
                        })
                    }),
            ))
        },
    );

    runtime.block_on(server)
}
