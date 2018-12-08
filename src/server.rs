use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::Instant;

use futures::sync::oneshot;
use futures::try_ready;
use log::{debug, info};
use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use crate::config::{HANDSHAKE_TIMEOUT, QUEUE_TIMEOUT, TRANSFER_TIMEOUT};
use crate::err;
use crate::future::FutureExt;
use crate::heartbeat;
use crate::magic;
use crate::stream::{spawn_idle, zip_left_then_right};
use crate::tcp;

pub fn run(public_addr: &SocketAddr, gateway_addr: &SocketAddr) -> Result<(), io::Error> {
    let mut runtime = Runtime::new()?;

    info!("Binding to public {}", public_addr);
    let public_connections = TcpListener::bind(public_addr)?.incoming();
    info!("Binding to gateway {}", gateway_addr);
    let gateway_connections = TcpListener::bind(gateway_addr)?.incoming();

    // drop public connections which wait for too long,
    // to avoid unlimited queuing when no client is connected
    let public_connections = spawn_idle(&runtime.handle(), |mut requests| {
        let mut public_connections = public_connections;
        let mut active_connection = None;
        stream::poll_fn(move || loop {
            active_connection = match active_connection {
                None => match try_ready!(public_connections.poll()) {
                    None => return Ok(None.into()),
                    Some(public) => Some((
                        public,
                        Delay::new(Instant::now() + QUEUE_TIMEOUT).map_err(err::to_io()),
                    )),
                },
                Some(_) => match requests.poll()? {
                    Async::NotReady => {
                        // no NLL :(
                        let (_, delay) = active_connection.as_mut().unwrap();
                        try_ready!(delay.poll());
                        debug!("Connection expired at idle");
                        None
                    }
                    Async::Ready(None) => return Ok(None.into()),
                    Async::Ready(Some(())) => {
                        return Ok(Some(active_connection.take().unwrap()).into())
                    }
                },
            };
        })
    });

    // early handshake: immediately kill unknown connections
    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            magic::read_from(gateway)
                .timeout(HANDSHAKE_TIMEOUT)
                .then(|r| match r {
                    Ok(gateway) => {
                        debug!("Early handshake succeeded");
                        Ok(Some(gateway))
                    }
                    Err(e) => {
                        debug!("Early handshake failed: {}", e);
                        Ok(None)
                    }
                })
        }).filter_map(|x| x);

    // heartbeat: so the client can tell if the connection drops
    // (and so the server doesn't accumulate a bunch of dead connections)
    let gateway_connections = spawn_idle(&runtime.handle(), |mut requests| {
        let mut gateway_connections = gateway_connections;
        let mut yield_requested = false;
        let mut active_heartbeat = None;
        stream::poll_fn(move || loop {
            if !yield_requested {
                match requests.poll()? {
                    Async::NotReady => {}
                    Async::Ready(None) => return Ok(None.into()),
                    Async::Ready(Some(())) => yield_requested = true,
                }
            }

            match gateway_connections.poll()? {
                Async::NotReady => {}
                Async::Ready(None) => return Ok(None.into()),
                Async::Ready(Some(gateway)) => {
                    let (stop_heartbeat, heartbeat_stopped) = oneshot::channel();
                    active_heartbeat = Some((
                        heartbeat::write_to(gateway, heartbeat_stopped.map_err(err::to_io())),
                        Some(stop_heartbeat),
                    ));
                }
            }

            let to_return;
            active_heartbeat = match &mut active_heartbeat {
                None => return Ok(Async::NotReady),
                Some((_, ref mut stop_heartbeat @ Some(_))) if yield_requested => {
                    match stop_heartbeat.take().unwrap().send(()) {
                        Ok(()) => continue,
                        Err(()) => {
                            to_return = None;
                            None
                        }
                    }
                }
                Some((ref mut heartbeat, _)) => match heartbeat.poll() {
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Ok(Async::Ready(gateway)) => {
                        debug!("Heartbeat completed");
                        assert!(yield_requested);
                        yield_requested = false;
                        to_return = Some(Ok(Some(gateway).into()));
                        None
                    }
                    Err(e) => {
                        debug!("Heartbeat failed: {}", e);
                        to_return = None;
                        None
                    }
                },
            };
            if let Some(to_return) = to_return {
                return to_return;
            }
        })
    });

    // late handshake: ensure that client hasn't disappeared some time after early handshake
    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            magic::read_from(gateway)
                .timeout(HANDSHAKE_TIMEOUT)
                .then(|r| match r {
                    Ok(gateway) => {
                        debug!("Late handshake succeeded");
                        Ok(Some(gateway))
                    }
                    Err(e) => {
                        debug!("Late handshake failed: {}", e);
                        Ok(None)
                    }
                })
        }).filter_map(|x| x);

    let active = Rc::new(AtomicUsize::new(0));

    let server = zip_left_then_right(public_connections, gateway_connections).for_each(
        move |((public, mut delay), gateway)| {
            if let Ok(Async::Ready(())) = delay.poll() {
                debug!("Connection expired at conjoinment");
                return Ok(());
            }
            info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
            let active = active.clone();
            Ok(spawn(
                tcp::conjoin(public, gateway)
                    .timeout_after_inactivity(TRANSFER_TIMEOUT)
                    .then(move |r| {
                        let active = active.fetch_sub(1, SeqCst) - 1;
                        Ok(match r {
                            Ok((down, up)) => info!("Closing ({} active): {}/{}", active, down, up),
                            Err(e) => info!("Closing ({} active): {}", active, e),
                        })
                    }),
            ))
        },
    );

    runtime.block_on(server)
}
