use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{job_req::JobDispatch, machine::MachineStatus};




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


/// message sent from the server to node
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    /// just a bash script for now (and just printed)
    Job {
        id: u64,
        dispatch: JobDispatch,
    }
}


pub async fn recieve_msg<'a, R: AsyncRead + Unpin, T: Deserialize<'a>>(mut stream: R, buf: &'a mut Vec<u8>) -> io::Result<T> {
    let num_bytes = stream.read_u32_le().await?;
    let mut stream = stream.take(num_bytes as u64);
    buf.clear();
    stream.read_to_end(buf).await?;
    let s = str::from_utf8(buf.as_slice()).map_err(io::Error::other)?;
    // dbg!(s);
    quick_xml::de::from_str(s).map_err(io::Error::other)
}

pub async fn send_msg<W: AsyncWrite + Unpin, T: Serialize>(mut stream: W, buf: &mut Vec<u8>, item: &T) -> io::Result<()> {
    buf.clear();
    quick_xml::se::to_utf8_io_writer(&mut *buf, item).map_err(io::Error::other)?;
    let len: u32 = buf.len().try_into().map_err(io::Error::other)?;
    stream.write_u32_le(len).await?;
    stream.write_all(&buf).await?;
    stream.flush().await
}




#[cfg(test)]
mod tests {
    use std::assert_matches;
    use crate::job_req::Exec;

    use super::*;

    #[tokio::test]
    async fn msg_roundtrip_client() {
        let (read_half, write_half) = tokio::io::simplex(5);
        let ((), msg) = tokio::join!(
            async move {
                send_msg(write_half, &mut Vec::new(), &ClientMsg::JobDone { id: 123, success: true, code: Some(0) }).await.unwrap();
            },
            async move {
                recieve_msg::<_, ClientMsg>(read_half, &mut Vec::new()).await.unwrap()
            }
        );

        assert_matches!(msg, ClientMsg::JobDone { id: 123, success: true, code: Some(0) });
    }

    #[tokio::test]
    async fn msg_roundtrip_server() {
        let (read_half, write_half) = tokio::io::simplex(5);
        let script_s = r#"echo "hello, world!""#;
        let exec = Exec::bash_script(script_s);
        let ((), msg) = tokio::join!(
            async move {
                send_msg(write_half, &mut Vec::new(), &ServerMsg::Job { id: 123, dispatch: JobDispatch { working_dir: crate::job_req::WorkingDir::Home, exec } }).await.unwrap();
            },
            async move {
                recieve_msg::<_, ServerMsg>(read_half, &mut Vec::new()).await.unwrap()
            }
        );

        assert_matches!(msg, ServerMsg::Job { id: 123, dispatch } if dispatch.exec.argv[2] == script_s);
    }
}
