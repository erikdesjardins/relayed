use crate::config::HEARTBEAT_TIMEOUT;
use std::convert::Infallible;
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{interval, timeout};

const HEARTBEAT: [u8; 1] = [0xdd];
const EXIT: [u8; 1] = [0x1c];

pub async fn read_from(mut reader: impl AsyncRead + Unpin) -> Result<(), io::Error> {
    let mut buf = [0; 1];
    loop {
        timeout(HEARTBEAT_TIMEOUT, reader.read_exact(&mut buf)).await??;
        match buf {
            HEARTBEAT => continue,
            EXIT => return Ok(()),
            _ => return Err(io::ErrorKind::InvalidData.into()),
        }
    }
}

pub async fn write_forever(mut writer: impl AsyncWrite + Unpin) -> Result<Infallible, io::Error> {
    let mut heartbeat = interval(HEARTBEAT_TIMEOUT / 2);
    loop {
        writer.write_all(&HEARTBEAT).await?;
        heartbeat.tick().await;
    }
}

pub async fn write_final(mut writer: impl AsyncWrite + Unpin) -> Result<(), io::Error> {
    writer.write_all(&EXIT).await
}
