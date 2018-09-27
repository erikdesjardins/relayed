use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use future::poll;
use tcp::Conjoin;

pub fn run(public: &SocketAddr, gateway: &SocketAddr) -> Result<(), io::Error> {
    info!("Binding to public {}", public);
    let public_connections = TcpListener::bind(public)?.incoming();
    info!("Binding to gateway {}", public);
    let gateway_connections = TcpListener::bind(gateway)?.incoming();

    let gateway_connections = gateway_connections
        .and_then(|gateway| {
            let magic_byte = poll(gateway, |gateway| {
                let mut buf = [0; 1];
                try_ready!(gateway.poll_read(&mut buf));
                match buf {
                    [42] => Ok(().into()),
                    _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
                }
            });

            let timeout = Delay::new(Instant::now() + Duration::from_secs(1)).then(|r| match r {
                Ok(()) => Err(io::Error::from(io::ErrorKind::TimedOut)),
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
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

    let server = public_connections
        .zip(gateway_connections)
        .for_each(|(public, gateway)| {
            info!("Spawning connection handler");
            Ok(spawn(
                // write to notify client that this connection is in use (even if the public client doesn't send anything)
                poll(gateway, |gateway| gateway.poll_write(&[42]))
                    .and_then(move |(gateway, _)| Conjoin::new(public, gateway))
                    .then(|r| Ok(match r {
                        Ok((down, up)) => info!("Closing connection: {} down, {} up", down, up),
                        Err(e) => info!("Closing connection: {}", e),
                    })),
            ))
        });

    Runtime::new()?.block_on(server)
}
