mod substitute;
mod random_copy;

use png_glitch::PngGlitch;
pub use substitute::Substitute;
pub use random_copy::RandomCopy;

pub trait Command {
    fn run(&self, png: &mut PngGlitch);
}