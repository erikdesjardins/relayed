use std::io::{self, ErrorKind::*};

use tokio::prelude::*;

const MAGIC: u8 = 42;

pub fn read_byte(reader: &mut impl AsyncRead) -> Poll<(), io::Error> {
    let mut buf = [0; 1];
    try_ready!(reader.poll_read(&mut buf));
    match buf {
        [MAGIC] => Ok(().into()),
        _ => Err(InvalidData.into()),
    }
}

pub fn write_byte(writer: &mut impl AsyncWrite) -> Poll<(), io::Error> {
    try_ready!(writer.poll_write(&[MAGIC]));
    Ok(().into())
}
