// Runs the live loop of the component in a Web Worker and times each stage with
// performance.now(). Stage names follow crates/png-glitch-bench (`browser/<stage>`) and the
// result has the schema of `png-glitch-bench --json`, so `png-glitch-bench compare` reads it.
import { SIZES, frameRgba } from "./generator.mjs";

// The same recipes as the native harness (`live/frame/*`).
export const RECIPES = {
  invert: "filters:\n  - type: invert\n",
  swap: "filters:\n  - type: transpose\n    magnitude: 0.3\n",
  regenerate: "filters:\n  - type: invert\n  - type: sub-filter\n",
};

const BUDGET_MS = 1000 / 30;

function summarize(samples) {
  const s = [...samples].sort((a, b) => a - b);
  const pick = (q) => (s.length ? s[Math.min(s.length, Math.max(1, Math.ceil(q * s.length))) - 1] : 0);
  return {
    count: s.length,
    mean_ms: s.length ? s.reduce((a, b) => a + b, 0) / s.length : 0,
    p50_ms: pick(0.5),
    p95_ms: pick(0.95),
    p99_ms: pick(0.99),
    max_ms: s.length ? s[s.length - 1] : 0,
  };
}

async function run({ size, frames, warmup, pool, recipe, commit, componentUrl }) {
  const [width, height] = SIZES[size];
  const component = await import(componentUrl);
  const { Session } = component.live;
  const recipeText = RECIPES[recipe];
  if (!recipeText) throw new Error(`unknown recipe ${recipe}`);

  // Setup, not timed.
  const raw = Array.from({ length: Math.max(pool, 1) }, (_, i) => new Uint8Array(frameRgba(width, height, i).buffer));
  const enc = { colorType: "rgba", bitDepth: 8, filter: { tag: "fixed", val: "paeth" } };
  const s = Session.fromPixels(width, height, raw[0], enc);
  s.setRecipe(recipeText, 0x5eedn, "per-frame");

  const canvas = typeof OffscreenCanvas === "undefined" ? null : new OffscreenCanvas(width, height);
  const ctx = canvas && canvas.getContext("2d", { willReadFrequently: true });
  const image = ctx ? new ImageData(new Uint8ClampedArray(raw[0].buffer.slice(0)), width, height) : null;
  const wrong = raw[0].subarray(0, raw[0].length - 4); // right size to lower, wrong length for the guest
  const target = new Uint8Array(raw[0].length);

  const names = ["frame", "reload-pixels", "apply", "render-rgba", "transfer-in", "draw-out", "copy-in-floor", "boundary-in", "snapshot"];
  const t = Object.fromEntries(names.map((n) => [n, []]));
  const now = () => performance.now();
  const memStart = [];
  let rendered = null;

  const total = warmup + frames;
  for (let i = 0; i < total; i++) {
    const timed = i >= warmup;
    const f = raw[i % raw.length];
    const t0 = now();
    s.reloadPixels(f);
    const t1 = now();
    s.apply();
    const t2 = now();
    rendered = s.renderRgba();
    const t3 = now();
    if (timed) {
      t["frame"].push(t3 - t0);
      t["reload-pixels"].push(t1 - t0);
      t["apply"].push(t2 - t1);
      t["render-rgba"].push(t3 - t2);
    }
    if (i === warmup - 1 || (warmup === 0 && i === 0)) memStart.push(component.__memoryBytes());

    if (timed) {
      // The cost of getting a frame in and out of the page (what VideoFrame.copyTo and a
      // canvas draw cost), outside the component.
      if (ctx) {
        const a = now();
        ctx.getImageData(0, 0, width, height);
        const b = now();
        ctx.putImageData(image, 0, 0);
        const c = now();
        t["transfer-in"].push(b - a);
        t["draw-out"].push(c - b);
      }
      // Lower bound of any copy-in: one memcpy of a frame in JS.
      const d = now();
      target.set(f);
      t["copy-in-floor"].push(now() - d);
      // Boundary cost without work: the frame is lowered into guest memory, then the guest
      // rejects it for its length.
      const e = now();
      try { s.reloadPixels(wrong); } catch { /* expected: invalid-image */ }
      t["boundary-in"].push(now() - e);
      // Compression runs here, off the real-time path; sampled every 10th frame.
      if ((i - warmup) % 10 === 0) {
        const g = now();
        s.snapshot();
        t["snapshot"].push(now() - g);
      }
    }
  }
  const memEnd = component.__memoryBytes();
  const mem0 = memStart.length ? memStart[0] : memEnd;

  const stages = names
    .filter((n) => t[n].length)
    .map((n) => ({
      name: `browser/${n}`,
      ...summarize(t[n]),
      // Not measured in a browser: the wasm linear memory size stands in for the live heap.
      alloc_bytes_per_iter: 0,
      live_bytes_start: mem0,
      live_bytes_end: memEnd,
      ...(n === "frame" ? { over_33ms_share: t[n].filter((x) => x > BUDGET_MS).length / t[n].length } : {}),
    }));
  return {
    meta: {
      target: "wasm32-wasip2-browser",
      simd128: true,
      rayon_num_threads: null,
      git_commit: commit ?? null,
      size,
      width,
      height,
      frames,
      warmup,
      pool: raw.length,
    },
    stages,
    extra: { recipe, wasm_memory_start: mem0, wasm_memory_end: memEnd, user_agent: navigator.userAgent },
  };
}

self.onmessage = async (e) => {
  try {
    self.postMessage({ ok: true, report: await run(e.data) });
  } catch (err) {
    self.postMessage({ ok: false, error: String(err?.stack ?? err) });
  }
};
