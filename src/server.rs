use std::io;
use std::net::SocketAddr;

use tokio::executor::current_thread::spawn;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;

use tcp::Conjoin;

pub fn run(public: &SocketAddr, gateway: &SocketAddr) -> Result<(), io::Error> {
    info!("Binding to public {}...", public);
    let public_connections = TcpListener::bind(public)?.incoming();

    info!("Binding to gateway {}...", gateway);
    let gateway_connections = TcpListener::bind(gateway)?.incoming();

    let server = public_connections
        .zip(gateway_connections)
        .for_each(|(public, gateway)| {
            match (public.peer_addr(), gateway.peer_addr()) {
                (Ok(p), Ok(g)) => info!("Transferring from {} to {}", p, g),
                (Err(e), _) | (_, Err(e)) => warn!("Error getting peer address: {}", e),
            }

            let conjoin = Conjoin::new(public, gateway).then(|r| {
                match r {
                    Ok((bytes_down, bytes_up)) => info!(
                        "Transfer complete: {} bytes down, {} bytes up",
                        bytes_down, bytes_up
                    ),
                    Err(e) => warn!("Transfer cancelled: {}", e),
                }
                Ok(())
            });

            spawn(conjoin);

            Ok(())
        });

    Runtime::new()?.block_on(server)
}
