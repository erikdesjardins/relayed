use std::net::SocketAddr;

use tokio;
use tokio::net::TcpStream;
use tokio::prelude::*;

use stream;
use tcp::conjoin;

pub fn run(gateway: SocketAddr, private: SocketAddr) {
    let server = stream::repeat_with(move || {
        TcpStream::connect(&gateway)
            .join(TcpStream::connect(&private))
            .and_then(|(gateway, private)| {
                match (gateway.peer_addr(), private.peer_addr()) {
                    (Ok(p), Ok(g)) => info!("Copying from {} to {}", p, g),
                    (Err(e), _) | (_, Err(e)) => warn!("Error getting peer address: {}", e),
                }
                conjoin(gateway, private)
            })
    }).for_each(|(bytes_out, bytes_in)| {
        info!("{} bytes out, {} bytes in", bytes_out, bytes_in);
        Ok(())
    })
        .map_err(|e| warn!("Error while copying: {}", e));

    tokio::run(server);
}
