use crate::config::{MAX_BUFFER_SIZE, MIN_BUFFER_SIZE};
use futures::future;
use futures::ready;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub fn conjoin(
    mut a: impl AsyncRead + AsyncWrite + Unpin,
    mut b: impl AsyncRead + AsyncWrite + Unpin,
) -> impl Future<Output = Result<(u64, u64), io::Error>> {
    let mut a_to_b = Buf::new();
    let mut b_to_a = Buf::new();
    future::poll_fn(move |cx| {
        // always attempt transfers in both directions
        let a_to_b = a_to_b.try_copy(&mut a, &mut b, cx)?;
        let b_to_a = b_to_a.try_copy(&mut b, &mut a, cx)?;
        // once both transfers are done, return transferred bytes
        Poll::Ready(Ok((ready!(a_to_b), ready!(b_to_a))))
    })
}

struct Buf {
    state: BufState,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Vec<u8>,
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
            buf: vec![0; MIN_BUFFER_SIZE],
        }
    }

    fn try_copy(
        &mut self,
        reader: &mut (impl AsyncRead + Unpin),
        writer: &mut (impl AsyncWrite + Unpin),
        cx: &mut Context<'_>,
    ) -> Poll<Result<u64, io::Error>> {
        loop {
            match self.state {
                BufState::ReadWrite => {
                    if self.pos == self.cap {
                        let mut buf = ReadBuf::new(&mut self.buf);
                        ready!(Pin::new(&mut *reader).poll_read(cx, &mut buf))?;
                        if buf.filled().is_empty() {
                            self.state = BufState::Shutdown;
                        } else {
                            self.pos = 0;
                            self.cap = buf.filled().len();
                        }
                    }

                    while self.pos < self.cap {
                        let i =
                            ready!(Pin::new(&mut *writer)
                                .poll_write(cx, &self.buf[self.pos..self.cap])?);
                        if i == 0 {
                            return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
                        } else {
                            self.pos += i;
                            self.amt += i as u64;
                            // if we read and write the full buffer at once, double it
                            if i == self.buf.len() && self.buf.len() < MAX_BUFFER_SIZE {
                                let double_len = self.buf.len() * 2;
                                self.buf.resize(double_len, 0);
                            }
                        }
                    }
                }
                BufState::Shutdown => {
                    ready!(Pin::new(&mut *writer).poll_shutdown(cx)?);
                    self.state = BufState::Done;
                }
                BufState::Done => {
                    return Poll::Ready(Ok(self.amt));
                }
            }
        }
    }
}
