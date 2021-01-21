use crate::backoff::Backoff;
use crate::config::{QUEUE_TIMEOUT, SERVER_ACCEPT_BACKOFF_SECS};
use crate::err::{AppliesTo, IoErrorExt};
use crate::heartbeat;
use crate::magic;
use crate::rw::conjoin;
use crate::stream::spawn_idle;
use futures::future::{select, Either};
use futures::stream;
use futures::StreamExt;
use pin_utils::pin_mut;
use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::LocalSet;
use tokio::time::error::Elapsed;
use tokio::time::{sleep, timeout};

async fn accept(listener: &mut TcpListener) -> TcpStream {
    let mut backoff = Backoff::new(SERVER_ACCEPT_BACKOFF_SECS);
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                backoff.reset();
                if let Err(e) = stream.set_nodelay(true) {
                    log::warn!("Failed to set nodelay: {}", e);
                    continue;
                }
                return stream;
            }
            Err(e) => match e.applies_to() {
                AppliesTo::Connection => log::info!("Aborted connection dropped: {}", e),
                AppliesTo::Listener => {
                    log::error!("Error accepting connections: {}", e);
                    let seconds = backoff.next();
                    log::warn!("Retrying in {} seconds", seconds);
                    sleep(Duration::from_secs(u64::from(seconds))).await;
                }
            },
        }
    }
}

async fn drain_queue(listener: &mut TcpListener) {
    loop {
        // timeout because we need to yield to receive the second queued conn
        // (listener.poll_recv() won't return Poll::Ready twice in a row,
        //  even if there are multiple queued connections)
        match timeout(Duration::from_millis(1), listener.accept()).await {
            Ok(Ok((_, _))) => log::info!("Queued conn dropped"),
            Ok(Err(e)) => match e.applies_to() {
                AppliesTo::Connection => log::info!("Queued conn dropped: {}", e),
                AppliesTo::Listener => break,
            },
            Err(e) => {
                let _: Elapsed = e;
                break;
            }
        }
    }
}

pub async fn run(
    local: &LocalSet,
    gateway_addr: &SocketAddr,
    public_addr: &SocketAddr,
) -> Result<(), io::Error> {
    let active = Rc::new(AtomicUsize::new(0));

    log::info!("Binding to gateway: {}", gateway_addr);
    let gateway_connections = TcpListener::bind(gateway_addr).await?;
    log::info!("Binding to public: {}", public_addr);
    let mut public_connections = TcpListener::bind(public_addr).await?;

    let gateway_connections = spawn_idle(local, |requests| {
        stream::unfold(
            (gateway_connections, requests),
            |(mut gateway_connections, mut requests)| async {
                loop {
                    let mut gateway = accept(&mut gateway_connections).await;

                    // early handshake: immediately kill unknown connections
                    match magic::read_from(&mut gateway).await {
                        Ok(()) => log::info!("Early handshake succeeded"),
                        Err(e) => {
                            log::info!("Early handshake failed: {}", e);
                            continue;
                        }
                    }

                    // heartbeat: so the client can tell if the connection drops
                    let token = {
                        let heartbeat = heartbeat::write_forever(&mut gateway);
                        pin_mut!(heartbeat);
                        match select(requests.next(), heartbeat).await {
                            Either::Left((Some(token), _)) => token,
                            Either::Left((None, _)) => return None,
                            Either::Right((Ok(i), _)) => match i {},
                            Either::Right((Err(e), _)) => {
                                log::info!("Heartbeat failed: {}", e);
                                continue;
                            }
                        }
                    };

                    return Some(((token, gateway), (gateway_connections, requests)));
                }
            },
        )
    });

    pin_mut!(gateway_connections);

    'public: loop {
        let public = accept(&mut public_connections).await;

        let gateway = loop {
            // drop public connections which wait for too long, to avoid unlimited queuing when no gateway is connected
            let mut gateway = match timeout(QUEUE_TIMEOUT, gateway_connections.next()).await {
                Ok(Some(gateway)) => gateway,
                Ok(None) => return Ok(()),
                Err(e) => {
                    let _: Elapsed = e;
                    log::info!("Public connection expired waiting for gateway");
                    drain_queue(&mut public_connections).await;
                    continue 'public;
                }
            };

            // finish heartbeat: do this as late as possible so clients can't send late handshake and disconnect
            match heartbeat::write_final(&mut gateway).await {
                Ok(()) => log::info!("Heartbeat completed"),
                Err(e) => {
                    log::info!("Heartbeat failed at finalization: {}", e);
                    continue;
                }
            }

            // late handshake: ensure that client hasn't disappeared some time after early handshake
            match magic::read_from(&mut gateway).await {
                Ok(()) => log::info!("Late handshake succeeded"),
                Err(e) => {
                    log::info!("Late handshake failed: {}", e);
                    continue;
                }
            }

            break gateway;
        };

        log::info!("Spawning ({} active)", active.fetch_add(1, Relaxed) + 1);
        let active = active.clone();
        local.spawn_local(async move {
            let done = conjoin(public, gateway).await;
            let active = active.fetch_sub(1, Relaxed) - 1;
            match done {
                Ok((down, up)) => log::info!("Closing ({} active): {}/{}", active, down, up),
                Err(e) => log::info!("Closing ({} active): {}", active, e),
            }
        });
    }
}
