use std::io;
use std::net::Shutdown;

use futures::try_ready;
use tokio::net::TcpStream;
use tokio::prelude::*;

use crate::rw;

pub fn conjoin(a: TcpStream, b: TcpStream) -> impl Future<Item = (u64, u64), Error = io::Error> {
    rw::conjoin(ShutdownOnClose(a), ShutdownOnClose(b))
}

struct ShutdownOnClose(TcpStream);

impl Read for ShutdownOnClose {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.0.read(buf)
    }
}

impl Write for ShutdownOnClose {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush()
    }
}

impl AsyncRead for ShutdownOnClose {}

impl AsyncWrite for ShutdownOnClose {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        try_ready!(<TcpStream as AsyncWrite>::shutdown(&mut self.0));
        TcpStream::shutdown(&self.0, Shutdown::Write)?;
        Ok(Async::Ready(()))
    }
}
