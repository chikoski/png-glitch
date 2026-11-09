// The JS frame generator equals the Rust one: CRC-32 of frames printed by
// `cargo test -p png-glitch-bench frame_checksums_for_the_browser_port`.
import { SIZES, crc32, frameRgba } from "./generator.mjs";

const EXPECTED = [
  ["360p", 0, 0x55193fed],
  ["720p", 0, 0x09a41720],
  ["1080p", 0, 0x79db4110],
  ["360p", 5, 0x58f77e6f],
];

let failed = false;
for (const [size, frame, want] of EXPECTED) {
  const [w, h] = SIZES[size];
  const got = crc32(frameRgba(w, h, frame));
  const ok = got === want;
  console.log(`${ok ? "ok  " : "FAIL"} ${size} frame ${frame}: ${got.toString(16).padStart(8, "0")}`);
  failed ||= !ok;
}
process.exit(failed ? 1 : 0);
