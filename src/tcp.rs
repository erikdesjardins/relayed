use std::io::{self, ErrorKind::*};
use std::net::Shutdown;

use tokio::net::TcpStream;
use tokio::prelude::*;

/// Connect two TCP streams, writing the output of each stream to the other
pub struct Conjoin {
    a: TcpStream,
    b: TcpStream,
    a_to_b: BufState,
    b_to_a: BufState,
}

impl Conjoin {
    pub fn new(a: TcpStream, b: TcpStream) -> Self {
        Self {
            a,
            b,
            a_to_b: BufState {
                read_done: false,
                pos: 0,
                cap: 0,
                amt: 0,
                buf: [0; 4096],
            },
            b_to_a: BufState {
                read_done: false,
                pos: 0,
                cap: 0,
                amt: 0,
                buf: [0; 4096],
            },
        }
    }
}

struct BufState {
    read_done: bool,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: [u8; 4096],
}

impl BufState {
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

impl Future for Conjoin {
    type Item = (u64, u64);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // always attempt transfers in both directions
        let a_to_b = self.a_to_b.try_copy(&mut self.a, &mut self.b);
        let b_to_a = self.b_to_a.try_copy(&mut self.b, &mut self.a);
        // once both transfers are done, return transferred bytes
        Ok((try_ready!(a_to_b), try_ready!(b_to_a)).into())
    }
}
