use tokio::prelude::*;

pub fn poll<S, T, E>(
    state: S,
    f: impl FnMut(&mut S) -> Poll<T, E>,
) -> impl Future<Item = (S, T), Error = E> {
    PollAdaptor(Some(state), f)
}

struct PollAdaptor<S, F>(Option<S>, F);

impl<S, T, E, F> Future for PollAdaptor<S, F>
where
    F: FnMut(&mut S) -> Poll<T, E>,
{
    type Item = (S, T);
    type Error = E;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let val = try_ready!((self.1)(self.0.as_mut().unwrap()));
        Ok((self.0.take().unwrap(), val).into())
    }
}

pub fn first_ok<
    T,
    R,
    E,
    Fut1: IntoFuture<Item = R, Error = E>,
    Fut2: IntoFuture<Item = R, Error = E>,
>(
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(T) -> Fut1,
    default: impl FnOnce() -> Fut2,
) -> impl Future<Item = R, Error = E> {
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
