use std::io;

use futures::try_ready;
use tokio::prelude::*;

use config::BUFFER_SIZE;

pub fn conjoin(
    mut a: impl AsyncRead + AsyncWrite,
    mut b: impl AsyncRead + AsyncWrite,
) -> impl Future<Item = (u64, u64), Error = io::Error> {
    let mut a_to_b = Buf::new();
    let mut b_to_a = Buf::new();
    future::poll_fn(move || {
        // always attempt transfers in both directions
        let a_to_b = a_to_b.try_copy(&mut a, &mut b);
        let b_to_a = b_to_a.try_copy(&mut b, &mut a);
        // once both transfers are done, return transferred bytes
        Ok((try_ready!(a_to_b), try_ready!(b_to_a)).into())
    })
}

struct Buf {
    state: BufState,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: [u8; BUFFER_SIZE],
}

enum BufState {
    ReadWrite,
    Shutdown,
    Done,
}

impl Buf {
    fn new() -> Self {
        Self {
            state: BufState::ReadWrite,
            pos: 0,
            cap: 0,
            amt: 0,
            buf: [0; BUFFER_SIZE],
        }
    }

    fn try_copy(
        &mut self,
        reader: &mut impl AsyncRead,
        writer: &mut impl AsyncWrite,
    ) -> Poll<u64, io::Error> {
        loop {
            match self.state {
                BufState::ReadWrite => {
                    if self.pos == self.cap {
                        let n = try_ready!(reader.poll_read(&mut self.buf));
                        if n == 0 {
                            self.state = BufState::Shutdown;
                        } else {
                            self.pos = 0;
                            self.cap = n;
                        }
                    }

                    while self.pos < self.cap {
                        let i = try_ready!(writer.poll_write(&self.buf[self.pos..self.cap]));
                        if i == 0 {
                            return Err(io::ErrorKind::WriteZero.into());
                        } else {
                            self.pos += i;
                            self.amt += i as u64;
                        }
                    }
                }
                BufState::Shutdown => {
                    try_ready!(writer.shutdown());
                    self.state = BufState::Done;
                }
                BufState::Done => {
                    return Ok(self.amt.into());
                }
            }
        }
    }
}
