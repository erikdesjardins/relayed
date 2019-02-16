use std::io;
use std::time::{Duration, Instant};

use futures::try_ready;
use tokio::prelude::*;
use tokio::timer::Delay;

use crate::err;

pub trait FutureExt: Future + Sized {
    fn timeout(self, time: Duration) -> Timeout<Self>
    where
        Self::Error: From<io::Error>,
    {
        Timeout {
            fut: self,
            delay: Delay::new(Instant::now() + time),
        }
    }
}

impl<T: Future> FutureExt for T {}

pub struct Timeout<Fut> {
    fut: Fut,
    delay: Delay,
}

impl<Fut> Future for Timeout<Fut>
where
    Fut: Future,
    Fut::Error: From<io::Error>,
{
    type Item = Fut::Item;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Async::Ready(val) = self.fut.poll()? {
            return Ok(val.into());
        }
        try_ready!(self.delay.poll().map_err(err::to_io()));
        Err(Fut::Error::from(io::ErrorKind::TimedOut.into()))
    }
}
