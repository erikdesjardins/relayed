use std::io;
use std::net::{self, SocketAddr};
use std::time::{Duration, Instant};

use tokio::executor::current_thread::spawn;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::reactor::Handle;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;

use backoff::Backoff;
use future::poll;
use stream;
use tcp::Conjoin;

pub fn run(
    gateway: Vec<SocketAddr>,
    private: Vec<SocketAddr>,
    retry: bool,
) -> Result<(), io::Error> {
    let backoff = Backoff::new(1..=64);

    let server = stream::repeat_with(|| {
        future::ok(())
            .and_then(|()| {
                let gateway = TcpStream::from_std(
                    net::TcpStream::connect(gateway.as_slice())?,
                    &Handle::default(),
                )?;
                info!("Connected to gateway {}", gateway.peer_addr()?);
                let private = TcpStream::from_std(
                    net::TcpStream::connect(private.as_slice())?,
                    &Handle::default(),
                )?;
                info!("Connected to private {}", private.peer_addr()?);
                Ok((gateway, private))
            }).and_then(|(gateway, private)| {
                poll(gateway, |gateway| gateway.poll_write(&[42]))
                    .and_then(|(gateway, _)| {
                        poll(gateway, |gateway| {
                            let mut buf = [0; 1];
                            try_ready!(gateway.poll_read(&mut buf));
                            match buf {
                                [42] => Ok(().into()),
                                _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
                            }
                        })
                    }).map(move |(gateway, _)| (gateway, private))
            }).and_then(|(gateway, private)| {
                info!("Spawning connection handler");
                Ok(spawn(Conjoin::new(gateway, private).then(|r| {
                    Ok(match r {
                        Ok((down, up)) => info!("Closing connection: {} down, {} up", down, up),
                        Err(e) => info!("Closing connection: {}", e),
                    })
                })))
            }).then(|r| match r {
                Ok(()) => {
                    backoff.reset();
                    future::Either::A(future::ok(()))
                }
                Err(ref e) if retry => {
                    error!("Opening connection failed: {}", e);
                    let seconds = backoff.get();
                    warn!("Retrying in {} seconds", seconds);
                    future::Either::B(
                        Delay::new(Instant::now() + Duration::from_secs(seconds as u64))
                            .map_err(|e| io::Error::new(io::ErrorKind::Other, e)),
                    )
                }
                Err(e) => future::Either::A(future::err(e)),
            })
    }).for_each(Ok);

    Runtime::new()?.block_on(server)
}
