#![no_main]
use libfuzzer_sys::fuzz_target;
use png_glitch::PngGlitch;
use png_glitch::presets::{Invert, Brighten};

fuzz_target!(|data: &[u8]| {
    // Attempt to parse the random data
    if let Ok(mut glitch) = PngGlitch::new(data.to_vec()) {
        // If parsing succeeded, try some operations
        
        // 1. Remove filters
        glitch.remove_filter();
        
        // 2. Apply some presets
        glitch.apply(Invert);
        glitch.apply(Brighten { strength: 100 });
        
        // 3. Try a transpose if the image has some height
        let height = glitch.height();
        if height > 2 {
            let mid = height / 2;
            glitch.transpose(0, mid, 1);
        }
        
        // 4. Encode it back
        let mut out = vec![];
        let _ = glitch.encode(&mut out);
    }
});
