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
