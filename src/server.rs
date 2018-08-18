use std::net::SocketAddr;

use tokio;
use tokio::net::TcpListener;
use tokio::prelude::*;

use tcp::conjoin;

pub fn run(public: &SocketAddr, gateway: &SocketAddr) {
    info!("Starting server...");

    info!("Binding to public {}...", public);
    let public_connections = TcpListener::bind(public)
        .expect("Bind to public")
        .incoming();

    info!("Binding to gateway {}...", gateway);
    let gateway_connections = TcpListener::bind(gateway)
        .expect("Bind to gateway")
        .incoming();

    let server = public_connections
        .zip(gateway_connections)
        .and_then(|(public, gateway)| {
            match (public.peer_addr(), gateway.peer_addr()) {
                (Ok(p), Ok(g)) => info!("Copying from {} to {}", p, g),
                (Err(e), _) | (_, Err(e)) => warn!("Error getting peer address: {}", e),
            }
            conjoin(public, gateway)
        })
        .for_each(|(bytes_out, bytes_in)| {
            info!("{} bytes out, {} bytes in", bytes_out, bytes_in);
            Ok(())
        })
        .map_err(|e| warn!("Error while copying: {}", e));

    tokio::run(server);
}
