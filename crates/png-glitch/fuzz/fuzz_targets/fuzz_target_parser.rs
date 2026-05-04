#![no_main]
use libfuzzer_sys::fuzz_target;
use png_glitch::png::parser::Parser;

fuzz_target!(|data: &[u8]| {
    // Target the raw parser directly to find edge cases in chunk handling
    let _ = Parser::parse(data);
});
