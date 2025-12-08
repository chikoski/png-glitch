use std::io::Write;

/// The trait provides a method to encode a PNG image.
pub trait Encode {
    /// The method encodes a PNG image and writes it to a buffer.
    /// The `buffer` parameter is a writable buffer.
    fn encode(&self, buffer: impl Write) -> anyhow::Result<()>;
}