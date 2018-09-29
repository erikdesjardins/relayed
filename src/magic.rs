use std::io::{self, ErrorKind::*};

use tokio::prelude::*;

use future::poll_with;

const MAGIC: u8 = 42;

pub fn read_from<T: AsyncRead>(reader: T) -> impl Future<Item = T, Error = io::Error> {
    poll_with(reader, |reader| {
        let mut buf = [0; 1];
        try_ready!(reader.poll_read(&mut buf));
        match buf {
            [MAGIC] => Ok(().into()),
            _ => Err(InvalidData.into()),
        }
    }).map(|(reader, ())| reader)
}

pub fn write_to<T: AsyncWrite>(writer: T) -> impl Future<Item = T, Error = io::Error> {
    poll_with(writer, |writer| writer.poll_write(&[MAGIC])).map(|(writer, _)| writer)
}
