use futures::stream;
use futures::{Stream, StreamExt};
use pin_utils::pin_mut;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;
use tokio::task::LocalSet;

/// Spawns a stream onto the local set to perform idle work.
/// This keeps polling the inner stream even when no item is demanded by the parent,
/// allowing it to keep making progress.
pub fn spawn_idle<T, S>(local: &LocalSet, f: impl FnOnce(Requests) -> S) -> impl Stream<Item = T>
where
    T: 'static,
    S: Stream<Item = (RequestToken, T)> + 'static,
{
    let (request, requests) = mpsc::channel(1);
    let (mut response, responses) = mpsc::channel(1);

    let idle = f(Requests(requests));
    local.spawn_local(async move {
        pin_mut!(idle);
        loop {
            match idle.next().await {
                Some((token, val)) => match response.send((ManuallyDrop::new(token), val)).await {
                    Ok(()) => continue,
                    Err(mpsc::error::SendError(_)) => return,
                },
                None => return,
            }
        }
    });

    stream::unfold(
        (request, responses, ManuallyDrop::new(RequestToken(()))),
        |(mut request, mut responses, token)| async {
            match request.send(token).await {
                Ok(()) => match responses.recv().await {
                    Some((token, val)) => Some((val, (request, responses, token))),
                    None => None,
                },
                Err(mpsc::error::SendError(_)) => None,
            }
        },
    )
}

pub struct RequestToken(());

impl Drop for RequestToken {
    fn drop(&mut self) {
        panic!("Deadlock: request token dropped");
    }
}

pub struct Requests(mpsc::Receiver<ManuallyDrop<RequestToken>>);

impl Stream for Requests {
    type Item = RequestToken;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Stream::poll_next(Pin::new(&mut self.0), cx).map(|p| p.map(ManuallyDrop::into_inner))
    }
}
