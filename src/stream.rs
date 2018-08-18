use tokio::prelude::*;

pub struct RepeatWith<F, Fut, T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Item = T, Error = E>,
{
    f: F,
    fut: Fut,
}

pub fn repeat_with<F, Fut, T, E>(mut f: F) -> RepeatWith<F, Fut, T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Item = T, Error = E>,
{
    RepeatWith { fut: f(), f }
}

impl<F, Fut, T, E> Stream for RepeatWith<F, Fut, T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Item = T, Error = E>,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.fut.poll() {
            Ok(Async::Ready(x)) => {
                self.fut = (self.f)();
                Ok(Async::Ready(Some(x)))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}
