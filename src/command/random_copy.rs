use crate::command::Command;
use png_glitch::PngGlitch;
use rand::{thread_rng, Rng};
use std::io::{Read, Write};

pub struct RandomCopy {
    times: u32,
}

impl RandomCopy {
    pub fn new(times: u32) -> RandomCopy {
        RandomCopy { times }
    }
}

impl Command for RandomCopy {
    fn run(&self, png: &mut PngGlitch) {
        let mut scan_lines = png.scan_lines();
        let mut rng = thread_rng();
        let index_range = 0..scan_lines.len();
        for _ in 0..self.times {
            let src = rng.gen_range(index_range.clone());
            let dest = rng.gen_range(index_range.clone());

            let src = &mut scan_lines[src];
            let filter_type = src.filter_type();
            let mut buffer = vec![];
            src.read_to_end(&mut buffer).unwrap();

            let dest = &mut scan_lines[dest];
            dest.write(&buffer).unwrap();
            dest.set_filter_type(filter_type);
        }
    }
}