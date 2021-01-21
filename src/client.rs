use crate::backoff::Backoff;
use crate::config::CLIENT_BACKOFF_SECS;
use crate::future::select_ok;
use crate::heartbeat;
use crate::magic;
use crate::rw::conjoin;
use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::task::LocalSet;
use tokio::time::sleep;

async fn connect(addrs: &[SocketAddr]) -> Result<TcpStream, io::Error> {
    let stream = select_ok(addrs.iter().map(TcpStream::connect)).await?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

pub async fn run(
    local: &LocalSet,
    gateway_addrs: &[SocketAddr],
    private_addrs: &[SocketAddr],
) -> ! {
    let mut backoff = Backoff::new(CLIENT_BACKOFF_SECS);
    let active = Rc::new(AtomicUsize::new(0));

    loop {
        let one_round = async {
            log::info!("Connecting to gateway");
            let mut gateway = connect(gateway_addrs).await?;

            log::info!("Sending early handshake");
            magic::write_to(&mut gateway).await?;

            log::info!("Waiting for end of heartbeat");
            heartbeat::read_from(&mut gateway).await?;

            log::info!("Sending late handshake");
            magic::write_to(&mut gateway).await?;

            log::info!("Connecting to private");
            let private = connect(private_addrs).await?;

            log::info!("Spawning ({} active)", active.fetch_add(1, Relaxed) + 1);
            let active = active.clone();
            local.spawn_local(async move {
                let done = conjoin(gateway, private).await;
                let active = active.fetch_sub(1, Relaxed) - 1;
                match done {
                    Ok((down, up)) => log::info!("Closing ({} active): {}/{}", active, down, up),
                    Err(e) => log::info!("Closing ({} active): {}", active, e),
                }
            });

            Ok::<(), io::Error>(())
        }
        .await;

        match one_round {
            Ok(()) => {
                backoff.reset();
            }
            Err(e) => {
                log::error!("Failed: {}", e);
                let seconds = backoff.next();
                log::warn!("Retrying in {} seconds", seconds);
                sleep(Duration::from_secs(u64::from(seconds))).await;
            }
        }
    }
}
