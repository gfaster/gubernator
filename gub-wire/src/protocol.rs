use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::machine::MachineStatus;




/// message sent from the client, excluding the startup [`crate::machine::MachineDesc`]
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    JobDone {
        id: u64,
        success: bool,
        code: Option<u8>,
    },
    Status(MachineStatus),
    Shutdown,
}


/// message sent from the server
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    /// just a bash script for now (and just printed)
    Spawn { id: u64, script: String }
}


pub async fn recieve_msg<'a, R: AsyncRead + Unpin, T: Deserialize<'a>>(mut stream: R, buf: &'a mut Vec<u8>) -> io::Result<T> {
    let num_bytes = stream.read_u32_le().await?;
    let mut stream = stream.take(num_bytes as u64);
    buf.clear();
    stream.read_to_end(buf).await?;
    postcard::from_bytes(buf.as_slice()).map_err(io::Error::other)
}

pub async fn send_msg<W: AsyncWrite + Unpin, T: Serialize>(mut stream: W, buf: &mut Vec<u8>, item: &T) -> io::Result<()> {
    buf.clear();
    *buf = postcard::to_extend(item, std::mem::take(buf)).map_err(io::Error::other)?;
    let len: u32 = buf.len().try_into().map_err(io::Error::other)?;
    stream.write_u32_le(len).await?;
    stream.write_all(&buf).await?;
    Ok(())
}
