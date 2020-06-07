use crate::backoff::Backoff;
use crate::config::{KEEPALIVE_TIMEOUT, QUEUE_TIMEOUT, SERVER_ACCEPT_BACKOFF_SECS};
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
use tokio::time::{delay_for, timeout, Elapsed};

async fn accept_conn(listener: &mut TcpListener) -> TcpStream {
    let mut backoff = Backoff::new(SERVER_ACCEPT_BACKOFF_SECS);
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                if let Err(e) = stream.set_nodelay(true) {
                    log::warn!("Failed to set nodelay: {}", e);
                    continue;
                }
                if let Err(e) = stream.set_keepalive(Some(KEEPALIVE_TIMEOUT)) {
                    log::warn!("Failed to set keepalive: {}", e);
                    continue;
                }
                return stream;
            }
            Err(e) => match e.kind() {
                io::ErrorKind::ConnectionRefused
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionReset => {
                    log::info!("Aborted connection dropped: {}", e);
                }
                _ => {
                    log::error!("Error accepting connections: {}", e);
                    let seconds = backoff.next();
                    log::warn!("Retrying in {} seconds", seconds);
                    delay_for(Duration::from_secs(u64::from(seconds))).await;
                }
            },
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
    let public_connections = TcpListener::bind(public_addr).await?;

    let public_connections = spawn_idle(local, |requests| {
        stream::unfold(
            (public_connections, requests),
            |(mut public_connections, mut requests)| async {
                loop {
                    let public = accept_conn(&mut public_connections).await;

                    // drop public connections which wait for too long,
                    // to avoid unlimited queuing when no client is connected
                    let (token, delay) = match select(requests.next(), delay_for(QUEUE_TIMEOUT))
                        .await
                    {
                        Either::Left((Some(token), delay)) => (token, delay),
                        Either::Left((None, _)) => return None,
                        Either::Right(((), _)) => {
                            log::info!("Connection expired at idle");
                            loop {
                                // timeout because we need to yield to receive the second queued conn
                                // (listener.poll_recv() won't return Poll::Ready twice in a row,
                                //  even if there are multiple queued connections)
                                match timeout(Duration::from_millis(1), public_connections.accept())
                                    .await
                                {
                                    Ok(Ok((_, _))) => log::info!("Queued conn dropped"),
                                    Ok(Err(e)) => {
                                        log::info!("Queued conn dropped with error: {}", e);
                                        // stop in case this is a resource error, which could lead to a hot loop
                                        break;
                                    }
                                    Err(e) => {
                                        let _: Elapsed = e;
                                        break;
                                    }
                                }
                            }
                            continue;
                        }
                    };

                    return Some(((token, (public, delay)), (public_connections, requests)));
                }
            },
        )
    });

    let gateway_connections = spawn_idle(local, |requests| {
        stream::unfold(
            (gateway_connections, requests),
            |(mut gateway_connections, mut requests)| async {
                loop {
                    let mut gateway = accept_conn(&mut gateway_connections).await;

                    // early handshake: immediately kill unknown connections
                    match magic::read_from(&mut gateway).await {
                        Ok(()) => log::info!("Early handshake succeeded"),
                        Err(e) => {
                            log::info!("Early handshake failed: {}", e);
                            continue;
                        }
                    }

                    // heartbeat: so the client can tell if the connection drops
                    // (and so the server doesn't accumulate a bunch of dead connections)
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
                    match heartbeat::write_final(&mut gateway).await {
                        Ok(()) => log::info!("Heartbeat completed"),
                        Err(e) => {
                            log::info!("Heartbeat failed at finalization: {}", e);
                            continue;
                        }
                    }

                    return Some(((token, gateway), (gateway_connections, requests)));
                }
            },
        )
    });

    pin_mut!(public_connections);
    pin_mut!(gateway_connections);

    loop {
        let (public, delay) = match public_connections.next().await {
            Some(x) => x,
            None => return Ok(()),
        };

        let gateway = loop {
            let mut gateway = match gateway_connections.next().await {
                Some(gateway) => gateway,
                None => return Ok(()),
            };

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

        if delay.is_elapsed() {
            log::info!("Connection expired at conjoinment");
            continue;
        }

        log::info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
        let active = active.clone();
        local.spawn_local(async move {
            let done = conjoin(public, gateway).await;
            let active = active.fetch_sub(1, SeqCst) - 1;
            match done {
                Ok((down, up)) => log::info!("Closing ({} active): {}/{}", active, down, up),
                Err(e) => log::info!("Closing ({} active): {}", active, e),
            }
        });
    }
}
