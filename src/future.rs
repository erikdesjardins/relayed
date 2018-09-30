use std::io::{self, ErrorKind::*};
use std::time::{Duration, Instant};

use tokio::prelude::*;
use tokio::timer::Delay;

pub fn poll_with<S, T, E>(
    state: S,
    mut f: impl FnMut(&mut S) -> Poll<T, E>,
) -> impl Future<Item = (S, T), Error = E> {
    let mut state = Some(state);
    future::poll_fn(move || {
        let val = try_ready!(f(state.as_mut().unwrap()));
        Ok((state.take().unwrap(), val).into())
    })
}

pub fn first_ok<T, R, E, Fut1, Fut2>(
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(T) -> Fut1,
    default: impl FnOnce() -> Fut2,
) -> impl Future<Item = R, Error = E>
where
    Fut1: IntoFuture<Item = R, Error = E>,
    Fut2: IntoFuture<Item = R, Error = E>,
{
    let mut iter = items.into_iter();
    match iter.next() {
        Some(first) => FirstOk::Full {
            fut: f(first).into_future(),
            iter,
            f,
        },
        None => FirstOk::Empty {
            fut: default().into_future(),
        },
    }
}

enum FirstOk<I, F, Fut1, Fut2> {
    Full { fut: Fut1, iter: I, f: F },
    Empty { fut: Fut2 },
}

impl<I, F, Fut1, Fut2> Future for FirstOk<I, F, Fut1::Future, Fut2>
where
    I: Iterator,
    F: FnMut(I::Item) -> Fut1,
    Fut1: IntoFuture,
    Fut2: Future<Item = Fut1::Item, Error = Fut1::Error>,
{
    type Item = Fut1::Item;
    type Error = Fut1::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self {
            FirstOk::Full { fut, iter, f } => match fut.poll() {
                Ok(a) => return Ok(a),
                Err(e) => match iter.next() {
                    None => return Err(e),
                    Some(v) => *fut = f(v).into_future(),
                },
            },
            FirstOk::Empty { fut } => return fut.poll(),
        }
        self.poll()
    }
}

pub fn timeout_after_inactivity<T>(time: Duration) -> impl Future<Item = T, Error = io::Error> {
    let mut poll_count = 0u32;
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
        Err(io::Error::from(TimedOut))
    })
}
