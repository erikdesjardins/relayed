use std::io::{self, ErrorKind::*};
use std::net::Shutdown;

use tokio::net::TcpStream;
use tokio::prelude::*;

pub fn conjoin(
    mut a: TcpStream,
    mut b: TcpStream,
) -> impl Future<Item = (u64, u64), Error = io::Error> {
    let mut a_to_b = BufState::new();
    let mut b_to_a = BufState::new();
    future::poll_fn(move || {
        // always attempt transfers in both directions
        let a_to_b = a_to_b.try_copy(&mut a, &mut b);
        let b_to_a = b_to_a.try_copy(&mut b, &mut a);
        // once both transfers are done, return transferred bytes
        Ok((try_ready!(a_to_b), try_ready!(b_to_a)).into())
    })
}

struct BufState {
    read_done: bool,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: [u8; 4096],
}

impl BufState {
    fn new() -> Self {
        Self {
            read_done: false,
            pos: 0,
            cap: 0,
            amt: 0,
            buf: [0; 4096],
        }
    }

    fn try_copy(&mut self, reader: &mut TcpStream, writer: &mut TcpStream) -> Poll<u64, io::Error> {
        loop {
            // If buffer is empty: read some data
            if self.pos == self.cap && !self.read_done {
                let n = try_ready!(reader.poll_read(&mut self.buf));
                if n == 0 {
                    self.read_done = true;
                } else {
                    self.pos = 0;
                    self.cap = n;
                }
            }

            // If buffer has data: write it out
            while self.pos < self.cap {
                let i = try_ready!(writer.poll_write(&self.buf[self.pos..self.cap]));
                if i == 0 {
                    return Err(io::Error::new(WriteZero, "writer accepted zero bytes"));
                } else {
                    self.pos += i;
                    self.amt += i as u64;
                }
            }

            // If transfer done: flush out data and shutdown the destination stream
            if self.pos == self.cap && self.read_done {
                try_ready!(writer.poll_flush());
                TcpStream::shutdown(writer, Shutdown::Write)?;
                return Ok(self.amt.into());
            }
        }
    }
}
