//! Feeds arbitrary bytes to the WebP RIFF container parser, round-trips it,
//! and runs every chunk-level mutation on the result.
#![no_main]
use libfuzzer_sys::fuzz_target;
use glitch_context::rand::SeedableRng;
use glitch_context::rand_chacha::ChaCha8Rng;
use webp_glitch::riff::RiffContainer;

fuzz_target!(|data: &[u8]| {
    let Ok(mut riff) = RiffContainer::parse(data) else {
        return;
    };

    // Whatever the parser accepted must encode, and the encoding must parse.
    let encoded = riff.encode();
    let reparsed = RiffContainer::parse(&encoded).expect("re-encoded RIFF must parse");
    assert_eq!(reparsed.chunks.len(), riff.chunks.len());

    for chunk in &riff.chunks {
        let _ = chunk.id_str();
    }

    // Use the chunk IDs actually present plus the well-known ones.
    let mut ids: Vec<[u8; 4]> = riff.chunks.iter().map(|c| c.id).collect();
    ids.extend([*b"VP8 ", *b"VP8L", *b"VP8X", *b"ALPH", *b"ICCP", *b"EXIF"]);
    ids.truncate(16);

    let mut rng = ChaCha8Rng::seed_from_u64(data.len() as u64);
    for (i, id) in ids.iter().enumerate() {
        let other = ids[(i + 1) % ids.len()];
        let _ = riff.find_chunk(id);
        let _ = riff.find_chunk_mut(id);
        let _ = riff.swap_chunks(id, &other);
        riff.corrupt_chunk(id, 0.5, &mut rng);
        let _ = riff.tamper_chunk_size(id, i32::MIN);
        let _ = riff.tamper_chunk_size(id, i32::MAX);
        let _ = riff.duplicate_chunk(id);
        let _ = riff.encode();
        riff.remove_chunk(id);
    }
    let _ = riff.encode();
});
