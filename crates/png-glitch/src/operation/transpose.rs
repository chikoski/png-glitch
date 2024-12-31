pub trait Transpose {
    fn transpose(&mut self, src: usize, dst: usize, lines: u32);
}