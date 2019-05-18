use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::Instant;

use futures::sync::oneshot;
use futures::try_ready;
use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use crate::config::{HANDSHAKE_TIMEOUT, KEEPALIVE_TIMEOUT, QUEUE_TIMEOUT};
use crate::err;
use crate::future::FutureExt;
use crate::heartbeat;
use crate::magic;
use crate::stream::{spawn_idle, zip_left_then_right};
use crate::tcp;

pub fn run(public_addr: &SocketAddr, gateway_addr: &SocketAddr) -> Result<(), io::Error> {
    let mut runtime = Runtime::new()?;

    log::info!("Binding to public {}", public_addr);
    let public_connections = TcpListener::bind(public_addr)?.incoming();
    log::info!("Binding to gateway {}", gateway_addr);
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
                Some((_, ref mut delay)) => match requests.poll()? {
                    Async::NotReady => {
                        try_ready!(delay.poll());
                        log::debug!("Connection expired at idle");
                        None
                    }
                    Async::Ready(None) => return Ok(None.into()),
                    Async::Ready(Some(token)) => {
                        return Ok(Some((token, active_connection.take().unwrap())).into());
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
                        log::debug!("Early handshake succeeded");
                        Ok(Some(gateway))
                    }
                    Err(e) => {
                        log::debug!("Early handshake failed: {}", e);
                        Ok(None)
                    }
                })
        })
        .filter_map(|x| x);

    // heartbeat: so the client can tell if the connection drops
    // (and so the server doesn't accumulate a bunch of dead connections)
    let gateway_connections = spawn_idle(&runtime.handle(), |mut requests| {
        let mut gateway_connections = gateway_connections;
        let mut current_request = None;
        let mut active_heartbeat = None;
        stream::poll_fn(move || loop {
            match requests.poll()? {
                Async::NotReady => {}
                Async::Ready(None) => return Ok(None.into()),
                Async::Ready(Some(token)) => current_request = Some(token),
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

            match (&mut current_request, &mut active_heartbeat) {
                (_, None) => return Ok(Async::NotReady),
                (Some(_), Some((_, ref mut stop_heartbeat @ Some(_)))) => {
                    match stop_heartbeat.take().unwrap().send(()) {
                        Ok(()) => continue,
                        Err(()) => active_heartbeat = None,
                    }
                }
                (ref mut token, Some((ref mut heartbeat, _))) => match heartbeat.poll() {
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Ok(Async::Ready(gateway)) => {
                        log::debug!("Heartbeat completed");
                        active_heartbeat = None;
                        return Ok(Some((token.take().unwrap(), gateway)).into());
                    }
                    Err(e) => {
                        log::debug!("Heartbeat failed: {}", e);
                        active_heartbeat = None;
                    }
                },
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
                        log::debug!("Late handshake succeeded");
                        Ok(Some(gateway))
                    }
                    Err(e) => {
                        log::debug!("Late handshake failed: {}", e);
                        Ok(None)
                    }
                })
        })
        .filter_map(|x| x);

    let active = Rc::new(AtomicUsize::new(0));

    let server = zip_left_then_right(public_connections, gateway_connections).for_each(
        move |((public, mut delay), gateway)| {
            if let Ok(Async::Ready(())) = delay.poll() {
                log::debug!("Connection expired at conjoinment");
                return Ok(());
            }
            public.set_keepalive(Some(KEEPALIVE_TIMEOUT))?;
            gateway.set_keepalive(Some(KEEPALIVE_TIMEOUT))?;
            log::info!("Spawning ({} active)", active.fetch_add(1, SeqCst) + 1);
            let active = active.clone();
            Ok(spawn(tcp::conjoin(public, gateway).then(move |r| {
                let active = active.fetch_sub(1, SeqCst) - 1;
                Ok(match r {
                    Ok((down, up)) => log::info!("Closing ({} active): {}/{}", active, down, up),
                    Err(e) => log::info!("Closing ({} active): {}", active, e),
                })
            })))
        },
    );

    runtime.block_on(server)
}
