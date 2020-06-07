use crate::config::HANDSHAKE_TIMEOUT;
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;

const MAGIC: [u8; 1] = [42];

pub async fn read_from(mut reader: impl AsyncRead + Unpin) -> Result<(), io::Error> {
    let mut buf = [0; 1];
    timeout(HANDSHAKE_TIMEOUT, reader.read_exact(&mut buf)).await??;
    match buf {
        MAGIC => Ok(()),
        _ => Err(io::ErrorKind::InvalidData.into()),
    }
}

pub async fn write_to(mut writer: impl AsyncWrite + Unpin) -> Result<(), io::Error> {
    writer.write_all(&MAGIC).await
}
