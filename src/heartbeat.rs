use std::io;
use std::time::Instant;

use futures::future::{Either, Loop};
use tokio::io::{read_exact, write_all};
use tokio::prelude::*;
use tokio::timer::Delay;

use crate::config::HEARTBEAT_TIMEOUT;
use crate::err;
use crate::future::FutureExt;

const HEARTBEAT: [u8; 1] = [0xdd];
const EXIT: [u8; 1] = [0x1c];

pub fn read_from<T: AsyncRead>(reader: T) -> impl Future<Item = T, Error = io::Error> {
    future::loop_fn(reader, |reader| {
        read_exact(reader, [0; 1])
            .timeout(HEARTBEAT_TIMEOUT)
            .and_then(|(reader, buf)| match buf {
                HEARTBEAT => Ok(Loop::Continue(reader)),
                EXIT => Ok(Loop::Break(reader)),
                _ => Err(io::ErrorKind::InvalidData.into()),
            })
    })
}

pub fn write_to<T: AsyncWrite>(
    writer: T,
    exit_when: impl Future<Item = (), Error = io::Error>,
) -> impl Future<Item = T, Error = io::Error> {
    future::loop_fn((writer, exit_when), |(writer, exit_when)| {
        write_all(writer, HEARTBEAT).and_then(|(writer, _)| {
            Delay::new(Instant::now() + HEARTBEAT_TIMEOUT / 2)
                .map_err(err::to_io())
                .select2(exit_when)
                .then(move |r| match r {
                    Ok(Either::A((_, exit_when))) => Ok(Loop::Continue((writer, exit_when))),
                    Ok(Either::B(((), _))) => Ok(Loop::Break(writer)),
                    Err(either) => Err(either.split().0),
                })
        })
    })
    .and_then(|writer| write_all(writer, EXIT).map(|(writer, _)| writer))
}
