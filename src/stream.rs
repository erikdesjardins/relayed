use tokio::prelude::*;

pub fn repeat_with<Fut, T, E>(mut f: impl FnMut() -> Fut) -> impl Stream<Item = T, Error = E>
where
    Fut: IntoFuture<Item = T, Error = E>,
{
    let mut fut = f().into_future();
    stream::poll_fn(move || {
        let val = try_ready!(fut.poll());
        fut = f().into_future();
        Ok(Some(val).into())
    })
}
