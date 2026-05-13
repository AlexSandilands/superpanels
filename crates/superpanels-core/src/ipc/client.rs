//! Synchronous IPC client used by the CLI and the GUI shell to talk to the
//! daemon. Wire format: 4-byte big-endian length prefix +
//! UTF-8 JSON body, capped at [`MAX_FRAME_BYTES`].
//!
//! Blocking I/O on a `std::os::unix::net::UnixStream` — no Tokio in this
//! module, so it composes with both the CLI's plain `main` and the GUI's
//! Tauri-worker thread without dragging an async runtime along.
//!
//! The typed [`ClientError`] keeps callers from string-matching transport
//! failures: the `library_thumbnail` confused-deputy guard needs
//! to distinguish a transport failure from the daemon's own logical
//! rejection of a path, and that distinction has to be structural.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use thiserror::Error;

use crate::ipc::{IpcRequest, IpcResponse, PROTOCOL_VERSION};

/// Write-side timeout. Applies to kernel-buffer backpressure on the local
/// socket — should never legitimately approach this limit.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Read-side timeout. Sized for the daemon's worst-case service time: an
/// `apply_profile` with image decode + KDE `evaluateScript` across N monitors
/// can exceed 10 s. Bounded so a wedged daemon can't hang clients forever.
pub const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// Maximum inbound-frame size. Daemon responses are tiny JSON; larger frames
/// signal a corrupt stream or hostile peer, and a `Vec` allocation of that
/// size should never be attempted.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("frame size exceeds cap: {0} bytes (max {MAX_FRAME_BYTES})")]
    FrameTooLarge(usize),
}

/// Try to connect to the daemon socket at `socket_path`. Returns `None` when
/// the file is absent or nothing is listening — the normal "daemon not
/// running" case, not an error worth surfacing.
pub fn try_connect(socket_path: &Path) -> Option<UnixStream> {
    UnixStream::connect(socket_path).ok()
}

/// Send `method` with `params` on `stream` and return the parsed response.
/// Sets read / write timeouts on every call so a stale stream can't hang.
pub fn call(
    stream: &mut UnixStream,
    method: &str,
    params: Value,
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
        let payload = b"hello IPC frame";
        write_frame(&mut writer, payload).unwrap();
        let got = read_frame(&mut reader).unwrap();
        assert_eq!(&got, payload);
    }

    #[test]
    fn frame_handles_empty_payload() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        write_frame(&mut writer, b"").unwrap();
        let got = read_frame(&mut reader).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn oversize_length_prefix_is_rejected_before_allocation() {
        // Hostile length prefix without any body bytes — must reject without
        // allocating MAX_FRAME_BYTES + 1 first.
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let oversize = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
        writer.write_all(&oversize.to_be_bytes()).unwrap();
        drop(writer);

        let err = read_frame(&mut reader).unwrap_err();
        assert!(
            matches!(err, ClientError::FrameTooLarge(n) if n == MAX_FRAME_BYTES + 1),
            "unexpected error: {err}"
        );
    }
}
