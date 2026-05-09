//! Synchronous IPC client for talking to the running daemon. Mirrors the CLI
//! client: 4-byte big-endian length + UTF-8 JSON, 1 MiB cap, with a short
//! write timeout (kernel buffer backpressure) and a long read timeout
//! (daemon service time — applies can take 10+ s).
//!
//! The Tauri commands run on a worker thread (`spawn_blocking`-equivalent),
//! so blocking I/O is fine here.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use superpanels_core::ipc::{IpcRequest, IpcResponse, PROTOCOL_VERSION};

const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ClientError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
}

pub(crate) fn try_connect(socket: &Path) -> Option<UnixStream> {
    UnixStream::connect(socket).ok()
}

pub(crate) fn call(
    stream: &mut UnixStream,
    method: &str,
    params: serde_json::Value,
) -> Result<IpcResponse, ClientError> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;

    let req = IpcRequest {
        v: PROTOCOL_VERSION,
        method: method.to_owned(),
        params,
    };
    let body = serde_json::to_vec(&req)?;
    write_frame(stream, &body)?;
    let frame = read_frame(stream)?;
    Ok(serde_json::from_slice(&frame)?)
}

fn write_frame(stream: &mut UnixStream, data: &[u8]) -> Result<(), ClientError> {
    let len = u32::try_from(data.len()).map_err(|_| ClientError::FrameTooLarge(data.len()))?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(data)?;
    Ok(())
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, ClientError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = usize::try_from(u32::from_be_bytes(len_buf))
        .map_err(|_| ClientError::FrameTooLarge(usize::MAX))?;
    if len > MAX_FRAME_BYTES {
        return Err(ClientError::FrameTooLarge(len));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body)?;
    Ok(body)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;

    #[test]
    fn frame_round_trips_through_socketpair() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        write_frame(&mut writer, b"hi").unwrap();
        let got = read_frame(&mut reader).unwrap();
        assert_eq!(&got, b"hi");
    }

    #[test]
    fn oversize_length_is_rejected() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let oversize = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
        writer.write_all(&oversize.to_be_bytes()).unwrap();
        drop(writer);
        let err = read_frame(&mut reader).unwrap_err();
        assert!(matches!(err, ClientError::FrameTooLarge(_)));
    }
}
