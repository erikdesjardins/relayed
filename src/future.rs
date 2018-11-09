use std::io;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use futures::{try_ready, Select};
use tokio::prelude::*;
use tokio::timer::Delay;

use err;

pub fn first_ok<Fut: IntoFuture>(
    items: impl IntoIterator<Item = Fut>,
) -> impl Future<Item = Fut::Item, Error = Fut::Error> {
    future::select_ok(items).map(|(x, _)| x)
}

pub trait FutureExt: Future + Sized {
    fn timeout_after_inactivity(self, time: Duration) -> Select<Self, Timeout<Self>>
    where
        Self::Error: From<io::Error>;
}

impl<T> FutureExt for T
where
    T: Future,
{
    fn timeout_after_inactivity(self, time: Duration) -> Select<Self, Timeout<Self>>
    where
        Self::Error: From<io::Error>,
    {
        self.select(Timeout {
            time,
            poll_count: 0,
            delay: Delay::new(Instant::now() + time),
            _fut: PhantomData,
        })
    }
}

pub struct Timeout<Fut> {
    time: Duration,
    poll_count: u64,
    delay: Delay,
    _fut: PhantomData<Fut>,
}

impl<Fut> Future for Timeout<Fut>
where
    Fut: Future,
    Fut::Error: From<io::Error>,
{
    type Item = Fut::Item;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.poll_count += 1;
        try_ready!(self.delay.poll().map_err(err::to_io()));
        // timer expired, but has anything else happened in the meantime?
        if self.poll_count > 1 {
            // something else has been polled: reset the timer
            self.poll_count = 0;
            self.delay = Delay::new(Instant::now() + self.time);
            // poll the new timer to start it--this should always be not ready,
            // but if it is somehow ready immediately that's still technically correct
            try_ready!(self.delay.poll().map_err(err::to_io()));
        }
        // nothing has been polled but the timer expiry, i.e. we've been inactive the whole time
        Err(Fut::Error::from(io::ErrorKind::TimedOut.into()))
    }
}
