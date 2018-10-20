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
use stream::zip_left_then_right;
use tcp;

pub fn run(public_addr: &SocketAddr, gateway_addr: &SocketAddr) -> Result<(), io::Error> {
    info!("Binding to public {}", public_addr);
    let public_connections = TcpListener::bind(public_addr)?.incoming();
    info!("Binding to gateway {}", gateway_addr);
    let gateway_connections = TcpListener::bind(gateway_addr)?.incoming();

    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            future::ok(gateway)
                .and_then(magic::write_to)
                .and_then(magic::read_from)
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

    // only poll gateway connections after a public connection is ready,
    // so we do the client handshake immediately before conjoining the two,
    // so we can be reasonably sure that the client didn't disappear after completing the handshake
    let server = zip_left_then_right(public_connections, gateway_connections)
        .for_each(move |(public, gateway)| {
            info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
            let active = active.clone();
            Ok(spawn(
                tcp::conjoin(public, gateway)
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
