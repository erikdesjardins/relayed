use futures::try_ready;
use tokio::prelude::*;

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
