use futures::try_ready;
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

pub fn zip_left_then_right<L, R, E>(
    mut left: impl Stream<Item = L, Error = E>,
    mut right: impl Stream<Item = R, Error = E>,
) -> impl Stream<Item = (L, R), Error = E> {
    let mut left_val = None;
    stream::poll_fn(move || loop {
        match left_val {
            None => match try_ready!(left.poll()) {
                None => return Ok(None.into()),
                Some(l) => left_val = Some(l),
            },
            Some(_) => match try_ready!(right.poll()) {
                None => return Ok(None.into()),
                Some(r) => return Ok(Some((left_val.take().unwrap(), r)).into()),
            },
        }
    })
}
