//! Length-prefixed IPC frame I/O on the Unix socket.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Hard cap on a single IPC frame body. Requests are tiny JSON objects;
/// anything larger is treated as a hostile or malformed sender.
pub(crate) const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub(crate) async fn read_frame(stream: &mut UnixStream) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = usize::try_from(u32::from_be_bytes(len_buf)).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame length overflows usize",
        )
    })?;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame length {len} exceeds {MAX_FRAME_BYTES}-byte cap"),
        ));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    Ok(body)
}

pub(crate) async fn write_frame(stream: &mut UnixStream, data: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(data.len()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "response exceeds 4 GiB")
    })?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(data).await?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_frame_rejects_oversize_length_before_allocating() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let oversize = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
        writer.write_all(&oversize.to_be_bytes()).await.unwrap();
        drop(writer);

        let result = read_frame(&mut reader).await;

        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("exceeds"),
            "unexpected error: {err}"
        );
    }
}
