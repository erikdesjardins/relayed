use futures::future::Executor;
use futures::sync::mpsc;
use futures::try_ready;
use tokio::prelude::*;

use crate::never::Never;

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

/// Spawns a stream onto the provided `executor` to perform idle work.
pub fn spawn_idle<T, E, S: Stream<Item = (RequestToken, T), Error = E>>(
    executor: &impl Executor<mpsc::Execute<S>>,
    f: impl FnOnce(Requests) -> S,
) -> impl Stream<Item = T, Error = E> {
    let (request, requests) = mpsc::channel(0);
    let mut request = request.sink_map_err(|_| panic!("inner channel never shuts down"));
    let requests = Requests(requests);

    let mut responses = mpsc::spawn(f(requests), executor, 0);

    stream::poll_fn({
        enum State {
            SendingRequest,
            FlushingRequest,
            WaitingForResponse,
        }
        let mut state = State::SendingRequest;
        move || loop {
            match state {
                State::SendingRequest => match request.start_send(RequestToken(()))? {
                    AsyncSink::NotReady(_) => return Ok(Async::NotReady),
                    AsyncSink::Ready => state = State::FlushingRequest,
                },
                State::FlushingRequest => {
                    try_ready!(request.poll_complete());
                    state = State::WaitingForResponse;
                }
                State::WaitingForResponse => {
                    let response = try_ready!(responses.poll());
                    state = State::SendingRequest;
                    return Ok(response.map(|(_, r)| r).into());
                }
            };
        }
    })
}

/// Unclonable token proving that a request was sent.
pub struct RequestToken(());

pub struct Requests(mpsc::Receiver<RequestToken>);

impl Stream for Requests {
    type Item = RequestToken;
    type Error = Never;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.0
            .poll()
            .map_err(|()| panic!("inner channel never shuts down"))
    }
}
