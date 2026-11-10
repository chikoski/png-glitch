// Runs the live loop of the component in a Web Worker and times each stage with
// performance.now(). Stage names follow crates/png-glitch-bench (`browser/<stage>`) and the
// result has the schema of `png-glitch-bench --json`, so `png-glitch-bench compare` reads it.
import { SIZES, frameRgba } from "./generator.mjs";
import { BUDGET_MS, nextFrame, perMinute, summarize } from "./pacing.mjs";

// The same recipes as the native harness (`live/frame/*`).
export const RECIPES = {
  invert: "filters:\n  - type: invert\n",
  swap: "filters:\n  - type: transpose\n    magnitude: 0.3\n",
  regenerate: "filters:\n  - type: invert\n  - type: sub-filter\n",
};

// Aux stages (canvas transfer, memcpy floor, boundary) in the paced mode run only when this much
// of the frame budget is left, so that they do not make the real-time loop miss a frame.
const AUX_SLACK_MS = 12;

async function run({ size, frames, warmup, pool, recipe, commit, componentUrl, duration, camera, cameraStream, cameraSettings, device, note }) {
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

  const names = ["frame", "reload-pixels", "apply", "render-rgba", "transfer-in", "draw-out", "copy-in-floor", "boundary-in", "snapshot", "camera-in"];
  const paced = Boolean(duration) || Boolean(camera);
  const durationMs = duration ? duration * 1000 : 0;
  // Preallocated samples in the paced mode (a long run must not grow JS arrays): one slot per
  // due frame, plus the start time of each frame for the per-minute summary.
  const capacity = paced ? Math.ceil((durationMs || frames * BUDGET_MS) / BUDGET_MS) + 2 : 0;
  const t = Object.fromEntries(names.map((n) => [n, paced && (n === "frame" || n === "camera-in") ? new Float64Array(capacity) : []]));
  const count = Object.fromEntries(names.map((n) => [n, 0]));
  const push = (n, v) => { if (paced && (n === "frame" || n === "camera-in")) t[n][count[n]++] = v; else { t[n].push(v); count[n]++; } };
  const starts = new Float64Array(paced ? capacity : 0);
  const now = () => performance.now();
  const memStart = [];
  let rendered = null;
  let droppedFrames = 0;
  let processed = 0;

  // One frame of the component; times go to the stage arrays when `timed`.
  const frameOnce = (f, timed, startAt) => {
    const t0 = now();
    s.reloadPixels(f);
    const t1 = now();
    s.apply();
    const t2 = now();
    rendered = s.renderRgba();
    const t3 = now();
    if (timed) {
      if (paced) starts[count["frame"]] = startAt;
      push("frame", t3 - t0);
      push("reload-pixels", t1 - t0);
      push("apply", t2 - t1);
      push("render-rgba", t3 - t2);
    }
  };
  // The cost of getting a frame in and out of the page (what VideoFrame.copyTo and a canvas
  // draw cost), outside the component, and two lower bounds.
  const aux = (f) => {
    if (ctx) {
      const a = now();
      ctx.getImageData(0, 0, width, height);
      const b = now();
      ctx.putImageData(image, 0, 0);
      const c = now();
      push("transfer-in", b - a);
      push("draw-out", c - b);
    }
    // Lower bound of any copy-in: one memcpy of a frame in JS.
    const d = now();
    target.set(f);
    push("copy-in-floor", now() - d);
    // Boundary cost without work: the frame is lowered into guest memory, then the guest
    // rejects it for its length, so there is no work in between.
    const e = now();
    try { s.reloadPixels(wrong); } catch { /* expected: invalid-image */ }
    push("boundary-in", now() - e);
  };
  const baseline = () => {
    // The first snapshot sizes the compressor's buffers; do it before the baseline so that the
    // steady-state check is about the frame loop (the snapshot is off the real-time path).
    s.snapshot();
    memStart.push(component.__memoryBytes());
  };

  // Warm-up always runs back to back.
  for (let i = 0; i < warmup; i++) {
    frameOnce(raw[i % raw.length], false, 0);
    if (i === warmup - 1) baseline();
  }

  let cameraPath = null;
  let cameraDelivered = 0;
  let elapsedEnd = 0;
  if (!paced) {
    for (let i = 0; i < frames; i++) {
      const f = raw[(warmup + i) % raw.length];
      frameOnce(f, true, 0);
      processed++;
      aux(f);
      // Compression runs here, off the real-time path; sampled every 10th frame.
      if (i % 10 === 0) {
        const g = now();
        s.snapshot();
        push("snapshot", now() - g);
      }
    }
  } else if (!camera) {
    // Frame k is due at start + k * BUDGET_MS. Run the latest due frame; frames we are behind on
    // are dropped, never queued. The snapshot is not sampled here: compression would take the
    // time of a frame or more.
    const start = now();
    let last = -1;
    for (;;) {
      const n = nextFrame(last, now() - start);
      if (n.index * BUDGET_MS >= durationMs) break;
      if (n.waitMs > 0) await new Promise((r) => setTimeout(r, n.waitMs));
      const f = raw[(warmup + n.index) % raw.length];
      droppedFrames += n.dropped;
      const at = now() - start;
      frameOnce(f, true, at);
      processed++;
      last = n.index;
      if (processed % 10 === 0 && (n.index + 1) * BUDGET_MS - (now() - start) > AUX_SLACK_MS) aux(f);
    }
    elapsedEnd = now() - start;
  } else {
    // The camera drives the pace: one frame per delivered VideoFrame (the track processor holds
    // at most one, so slow processing drops older frames). Not reproducible.
    const reader = cameraStream.getReader();
    const buf = new Uint8Array(width * height * 4);
    const toRgba = async (frame) => {
      if (cameraPath !== "canvas" && frame.displayWidth === width && frame.displayHeight === height) {
        try {
          await frame.copyTo(buf, { format: "RGBA" });
          cameraPath = "copyTo-rgba";
          return;
        } catch { cameraPath = "canvas"; }
      }
      cameraPath ??= "canvas";
      if (!ctx) throw new Error("no OffscreenCanvas for the camera canvas path");
      ctx.drawImage(frame, 0, 0, width, height);
      buf.set(new Uint8Array(ctx.getImageData(0, 0, width, height).data.buffer));
    };
    const start = now();
    let warmed = 0;
    let lastTs = null;
    const cameraIntervalUs = 1e6 / (cameraSettings?.frameRate || 30); // the rate the camera delivers
    for (;;) {
      const elapsed = now() - start;
      if ((durationMs && elapsed >= durationMs) || (!durationMs && processed >= frames)) break;
      const { value: frame, done } = await reader.read();
      if (done) break;
      const a = now();
      try {
        // Frames that the processor dropped show as gaps in the timestamps (in microseconds).
        if (lastTs !== null && frame.timestamp > lastTs) droppedFrames += Math.max(0, Math.round((frame.timestamp - lastTs) / cameraIntervalUs) - 1);
        lastTs = frame.timestamp;
        cameraDelivered++;
        await toRgba(frame);
      } finally {
        frame.close();
      }
      const inMs = now() - a;
      // The camera's own first frames are not timed (the component's warm-up ran above).
      if (warmed < warmup) { warmed++; frameOnce(buf, false, 0); continue; }
      push("camera-in", inMs);
      frameOnce(buf, true, a - start);
      processed++;
    }
    elapsedEnd = now() - start;
    await reader.cancel().catch(() => {});
  }
  if (!memStart.length) baseline();
  const memEnd = component.__memoryBytes();
  const mem0 = memStart.length ? memStart[0] : memEnd;

  const stages = names
    .filter((n) => count[n])
    .map((n) => ({
      name: `browser/${n}`,
      ...summarize(t[n], count[n]),
      // Not measured in a browser: the wasm linear memory size stands in for the live heap.
      alloc_bytes_per_iter: 0,
      live_bytes_start: mem0,
      live_bytes_end: memEnd,
      ...(n === "frame" ? { over_33ms_share: Array.from(t[n].subarray ? t[n].subarray(0, count[n]) : t[n]).filter((x) => x > BUDGET_MS).length / count[n] } : {}),
    }));
  const battery = device?.battery ?? null;
  return {
    meta: {
      target: "wasm32-wasip2-browser",
      simd128: true,
      rayon_num_threads: null,
      git_commit: commit ?? null,
      size,
      width,
      height,
      frames: processed,
      warmup,
      pool: raw.length,
    },
    stages,
    extra: {
      recipe,
      wasm_memory_start: mem0,
      wasm_memory_end: memEnd,
      user_agent: navigator.userAgent,
      cross_origin_isolated: self.crossOriginIsolated === true,
      hardware_concurrency: navigator.hardwareConcurrency ?? null,
      device_memory: navigator.deviceMemory ?? null,
      ...(device ? { screen: device.screen, battery, page_cross_origin_isolated: device.cross_origin_isolated } : {}),
      note: note ?? null,
      mode: camera ? "camera" : duration ? "paced" : "back-to-back",
      ...(paced
        ? {
            duration_s: elapsedEnd / 1000,
            dropped_frames: droppedFrames,
            per_minute: perMinute(starts, t["frame"], count["frame"]),
            snapshot: "off in the paced mode",
          }
        : {}),
      ...(camera
        ? { camera: { settings: cameraSettings ?? null, path: cameraPath, delivered_frames: cameraDelivered } }
        : {}),
    },
  };
}

self.onmessage = async (e) => {
  try {
    self.postMessage({ ok: true, report: await run(e.data) });
  } catch (err) {
    self.postMessage({ ok: false, error: String(err?.stack ?? err) });
  }
};
