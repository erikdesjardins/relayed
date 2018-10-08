use std::io::{self, ErrorKind::*};
use std::time::{Duration, Instant};

use tokio::prelude::*;
use tokio::timer::Delay;

pub fn first_ok<Fut: IntoFuture>(
    items: impl IntoIterator<Item = Fut>,
) -> impl Future<Item = Fut::Item, Error = Fut::Error> {
    future::select_ok(items).map(|(x, _)| x)
}

pub fn timeout_after_inactivity<T>(time: Duration) -> impl Future<Item = T, Error = io::Error> {
    let mut poll_count = 0u64;
    let mut delay = Delay::new(Instant::now() + time);
    future::poll_fn(move || {
        poll_count += 1;
        try_ready!(delay.poll().map_err(|e| io::Error::new(Other, e)));
        // timer expired, but has anything else happened in the meantime?
        if poll_count > 1 {
            // something else has been polled: reset the timer
            poll_count = 0;
            delay = Delay::new(Instant::now() + time);
            // poll the new timer to start it--this should always be not ready,
            // but if it is somehow ready immediately that's still technically correct
            try_ready!(delay.poll().map_err(|e| io::Error::new(Other, e)));
        }
        // nothing has been polled but the timer expiry, i.e. we've been inactive the whole time
        Err(TimedOut.into())
    })
}
