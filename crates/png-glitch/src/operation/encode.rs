use std::io::Write;

pub trait Encode {
    fn encode(&self, buffer: impl Write) -> anyhow::Result<()>;
}