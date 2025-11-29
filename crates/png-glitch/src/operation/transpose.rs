/// The trait provides a method to copy a chunk of scan lines from a location to another.
pub trait Transpose {
    /// The method copies a chunk of scan lines.
    /// The `src` parameter specifies the starting line of the chunk to be copied.
    /// The `dst` parameter specifies the location where the chunk is pasted.
    /// The `lines` parameter specifies the number of lines to be copied.
    fn transpose(&mut self, src: usize, dst: usize, lines: u32);
}