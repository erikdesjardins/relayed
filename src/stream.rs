use futures::stream;
use futures::{Stream, StreamExt};
use pin_utils::pin_mut;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::{mpsc, oneshot};
use tokio::task::LocalSet;

/// Spawns a stream onto the local set to perform idle work.
/// This keeps polling the inner stream even when no item is demanded by the parent,
/// allowing it to keep making progress.
pub fn spawn_idle<T, S>(local: &LocalSet, f: impl FnOnce(Requests<T>) -> S) -> impl Stream<Item = T>
where
    T: 'static,
    S: Stream<Item = (RequestToken<T>, T)> + 'static,
{
    let (request, requests) = mpsc::channel(1);

    let idle = f(Requests(requests));
    local.spawn_local(async move {
        pin_mut!(idle);
        loop {
            match idle.next().await {
                Some((RequestToken(chan), val)) => match chan.send(val) {
                    Ok(()) => continue,
                    Err(_) => return,
                },
                None => return,
            }
        }
    });

    stream::unfold(request, |mut request| async {
        loop {
            let (respond, response) = oneshot::channel();
            match request.send(RequestToken(respond)).await {
                Ok(()) => match response.await {
                    Ok(val) => return Some((val, request)),
                    Err(_) => continue,
                },
                Err(mpsc::error::SendError(_)) => return None,
            }
        }
    })
}

pub struct RequestToken<T>(oneshot::Sender<T>);

pub struct Requests<T>(mpsc::Receiver<RequestToken<T>>);

impl<T> Stream for Requests<T> {
    type Item = RequestToken<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Stream::poll_next(Pin::new(&mut self.0), cx)
    }
}
