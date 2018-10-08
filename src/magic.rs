use std::io::{self, ErrorKind::*};

use tokio::io::{read_exact, write_all};
use tokio::prelude::*;

const MAGIC: [u8; 1] = [42];

pub fn read_from<T: AsyncRead>(reader: T) -> impl Future<Item = T, Error = io::Error> {
    read_exact(reader, [0; 1]).and_then(|(reader, buf)| match buf {
        MAGIC => Ok(reader),
        _ => Err(InvalidData.into()),
    })
}

pub fn write_to<T: AsyncWrite>(writer: T) -> impl Future<Item = T, Error = io::Error> {
    write_all(writer, MAGIC).map(|(writer, _)| writer)
}
