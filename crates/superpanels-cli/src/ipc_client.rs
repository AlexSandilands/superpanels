//! Synchronous IPC client for communicating with the running daemon (`SPEC.md` §5.3).
//!
//! Uses `std::os::unix::net::UnixStream` (blocking I/O) so it works in the
//! CLI process without a Tokio runtime.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use superpanels_core::ipc::{IpcRequest, IpcResponse, PROTOCOL_VERSION};

// Write protects against kernel-buffer backpressure on the local socket and
// can stay short. Read has to cover the daemon's worst-case service time —
// an apply with image decode + KDE evaluateScript across N monitors easily
// exceeds 10 s — so it is much larger but still bounded so a wedged daemon
// can't hang the CLI forever.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// Hard cap on inbound frame bodies. Daemon responses are tiny JSON; larger
/// frames signal a corrupt stream or hostile peer, and a `Vec` allocation of
/// that size should not be attempted.
const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Try to connect to the daemon socket at `socket_path`.
///
/// Returns `None` if the file doesn't exist or nothing is listening — this is
/// the normal "daemon not running" case; it is not an error.
pub(crate) fn try_connect(socket_path: &Path) -> Option<UnixStream> {
    UnixStream::connect(socket_path).ok()
}

/// Send `method` with `params` to `stream` and return the parsed response.
pub(crate) fn call(
    stream: &mut UnixStream,
    method: &str,
    params: serde_json::Value,
) -> Result<IpcResponse> {
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .context("setting socket read timeout")?;
    stream
        .set_write_timeout(Some(WRITE_TIMEOUT))
        .context("setting socket write timeout")?;

    let req = IpcRequest {
        v: PROTOCOL_VERSION,
        method: method.to_owned(),
        params,
    };
    let body = serde_json::to_vec(&req).context("serialising IPC request")?;
    write_frame(stream, &body).context("writing IPC frame")?;

    let frame = read_frame(stream).context("reading IPC response")?;
    serde_json::from_slice(&frame).context("parsing IPC response")
}

fn write_frame(stream: &mut UnixStream, data: &[u8]) -> Result<()> {
    let len = u32::try_from(data.len()).context("request body exceeds 4 GiB")?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(data)?;
    Ok(())
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len =
        usize::try_from(u32::from_be_bytes(len_buf)).context("frame length overflows usize")?;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame length {len} exceeds {MAX_FRAME_BYTES}-byte cap"),
        )
        .into());
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body)?;
    Ok(body)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests — failure is a test bug
mod tests {
    use super::*;

    #[test]
    fn frame_round_trips_through_socketpair() {
        // Arrange
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let payload = b"hello IPC frame";

        // Act
        write_frame(&mut writer, payload).unwrap();
        let got = read_frame(&mut reader).unwrap();

        // Assert
        assert_eq!(&got, payload);
    }

    #[test]
    fn frame_with_oversize_length_is_rejected_before_allocation() {
        // Arrange — write a hostile length prefix without any body bytes.
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let oversize = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
        writer.write_all(&oversize.to_be_bytes()).unwrap();
        // Close writer to ensure the reader doesn't block on body bytes.
        drop(writer);

        // Act
        let result = read_frame(&mut reader);

        // Assert — InvalidData chained from io::Error.
        let err = result.unwrap_err();
        let chain = format!("{err:#}");
        assert!(
            chain.contains("exceeds") && chain.contains("cap"),
            "unexpected error: {chain}"
        );
    }

    #[test]
    fn frame_handles_empty_payload() {
        // Arrange
        let (mut writer, mut reader) = UnixStream::pair().unwrap();

        // Act
        write_frame(&mut writer, b"").unwrap();
        let got = read_frame(&mut reader).unwrap();

        // Assert
        assert!(got.is_empty());
    }
}
