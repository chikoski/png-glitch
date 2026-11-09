// Port of `frame_rgba` and `Size` from crates/png-glitch-bench/src/fixture.rs. The bytes must be
// identical (test-generator.mjs checks CRC-32s printed by the Rust test
// `frame_checksums_for_the_browser_port`). Runs in a page, a worker and Node.

export const SIZES = {
  "360p": [640, 360],
  "720p": [1280, 720],
  "1080p": [1920, 1080],
};

const SEED = 0x706e67206265_6e63n;
const GAMMA = [0x9e3779b9, 0x7f4a7c15]; // [hi, lo]
const C1 = [0xbf58476d, 0x1ce4e5b9];
const C2 = [0x94d049bb, 0x133111eb];
const U64 = (1n << 64n) - 1n;

/** Low 64 bits of a * b for [hi, lo] pairs of u32. */
function mul64(ah, al, bh, bl) {
  const a0 = al & 0xffff, a1 = al >>> 16, b0 = bl & 0xffff, b1 = bl >>> 16;
  const p00 = a0 * b0, p01 = a0 * b1, p10 = a1 * b0, p11 = a1 * b1;
  const mid = p01 + p10 + Math.floor(p00 / 65536);
  const lo = ((mid % 65536) * 65536 + (p00 % 65536)) >>> 0;
  const hi = (p11 + Math.floor(mid / 65536) + Math.imul(ah, bl) + Math.imul(al, bh)) >>> 0;
  return [hi, lo];
}

/** Frame `index` as RGBA bytes: gradients, two moving shapes and low-amplitude noise. */
export function frameRgba(width, height, index) {
  const w = width, h = height, t = index;
  const start = (SEED ^ ((BigInt(index) * 0xd6e8feb86659fd93n) & U64)) & U64;
  let sh = Number(start >> 32n) >>> 0;
  let sl = Number(start & 0xffffffffn) >>> 0;
  const disc = [(Math.floor(w / 4) + t * 7) % Math.max(w, 1), (Math.floor(h / 2) + t * 3) % Math.max(h, 1), Math.floor(Math.max(h, 8) / 6)];
  const bx = (((Math.floor((w * 3) / 4) - t * 5) % w) + w) % w;
  const box = [bx, Math.floor(h / 3), Math.floor(Math.max(w, 8) / 10), Math.floor(Math.max(h, 8) / 8)];
  const out = new Uint8ClampedArray(width * height * 4);
  const wh = Math.max(w + h, 1), w1 = Math.max(w, 1), h1 = Math.max(h, 1);
  let o = 0;
  for (let y = 0; y < h; y++) {
    const g0 = Math.floor((y * 255) / h1);
    for (let x = 0; x < w; x++) {
      let r = Math.floor((x * 255) / w1);
      let g = g0;
      let b = Math.floor(((x + y + t * 2) * 255) / wh);
      const dx = x - disc[0], dy = y - disc[1];
      if (dx * dx + dy * dy < disc[2] * disc[2]) { r = 230; g = 190; b = 160; }
      if (Math.abs(x - box[0]) < box[2] && Math.abs(y - box[1]) < box[3]) { r = 40; g = 90; b = 200; }
      // SplitMix64: state += GAMMA; z = mix(state); only the low 32 bits of the output are used.
      const lo = (sl + GAMMA[1]) >>> 0;
      sh = (sh + GAMMA[0] + (lo < sl ? 1 : 0)) >>> 0;
      sl = lo;
      let zh = sh, zl = sl;
      // z ^= z >> 30
      let th = zh >>> 30, tl = ((zh << 2) | (zl >>> 30)) >>> 0;
      zh = (zh ^ th) >>> 0; zl = (zl ^ tl) >>> 0;
      [zh, zl] = mul64(zh, zl, C1[0], C1[1]);
      // z ^= z >> 27
      th = zh >>> 27; tl = ((zh << 5) | (zl >>> 27)) >>> 0;
      zh = (zh ^ th) >>> 0; zl = (zl ^ tl) >>> 0;
      [zh, zl] = mul64(zh, zl, C2[0], C2[1]);
      // n = z ^ (z >> 31); low 32 bits only
      const n = (zl ^ ((zh << 1) | (zl >>> 31))) >>> 0;
      out[o++] = r + ((n & 7) - 3);
      out[o++] = g + (((n >>> 8) & 7) - 3);
      out[o++] = b + (((n >>> 16) & 7) - 3);
      out[o++] = 255;
    }
  }
  return out;
}

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

export function crc32(bytes) {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
