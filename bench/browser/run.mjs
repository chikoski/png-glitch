// npm run bench -- --size 1080p --frames 300 [--warmup 10] [--pool 8] [--recipe swap]
//                  [--json out/browser.json] [--browser-channel chrome] [--commit SHA]
//                  [--duration SECONDS] [--camera] [--note TEXT] [--port N]
//                  [--android | --device]
//
// Runs the component's live loop in a Web Worker of headless Chrome and prints a table. The
// JSON has the schema of `png-glitch-bench --json`.
//   --duration S  sustained run: frames paced at 30 fps for S seconds, late frames dropped (#222)
//   --camera      frames from getUserMedia (paced by the camera; implies the paced mode)
//   --android     drive Chrome on an Android device over adb with Playwright (experimental)
//   --device      serve the page, print the URL for the phone, wait for its POST /report
import { chromium } from "playwright-core";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { dirname } from "node:path";
import { serve } from "./server.mjs";

const argv = process.argv.slice(2);
const flag = (name) => argv.includes(`--${name}`);
const opt = (name, fallback) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : fallback;
};

export async function launch() {
  const channel = opt("browser-channel", undefined);
  const executablePath = opt("browser-path", process.env.CHROME_PATH ?? (existsSync("/opt/pw-browsers/chromium") ? "/opt/pw-browsers/chromium" : undefined));
  return chromium.launch({
    headless: true,
    ...(channel ? { channel } : executablePath ? { executablePath } : {}),
    args: [
      "--no-sandbox",
      "--enable-features=SharedArrayBuffer",
      "--enable-precise-memory-info",
      // A fake camera, so that --camera can be tried without hardware.
      ...(flag("camera") ? ["--use-fake-device-for-media-stream", "--use-fake-ui-for-media-stream"] : []),
    ],
  });
}

const options = {
  size: opt("size", "720p"),
  frames: Number(opt("frames", "60")),
  warmup: Number(opt("warmup", "10")),
  pool: Number(opt("pool", "8")),
  recipe: opt("recipe", "swap"),
  commit: opt("commit", process.env.BENCH_GIT_COMMIT ?? null),
  dist: opt("dist", "dist"),
  duration: opt("duration", undefined) === undefined ? undefined : Number(opt("duration")),
  camera: flag("camera"),
  note: opt("note", undefined) ?? null,
};
const port = Number(opt("port", "0"));
const android = flag("android");
const deviceMode = flag("device");

if (options.duration !== undefined && !(options.duration > 0)) {
  console.error("--duration must be a positive number of seconds");
  process.exit(2);
}
if ((android || deviceMode) && !port) {
  console.error("--android and --device need --port N, the port that `adb reverse tcp:N tcp:N` forwards");
  process.exit(2);
}

if (!(options.warmup >= 1)) {
  console.error("--warmup must be at least 1: the steady-state memory check needs a baseline after warm-up");
  process.exit(2);
}

function table(r) {
  const m = r.meta;
  let out = `target=${m.target} simd128=${m.simd128} commit=${m.git_commit ?? "-"} size=${m.size} (${m.width}x${m.height}) frames=${m.frames} warmup=${m.warmup} pool=${m.pool} recipe=${r.extra.recipe}\n`;
  out += `${r.extra.user_agent}\n\n`;
  out += `${"stage".padEnd(26)} ${"n".padStart(5)} ${"mean ms".padStart(9)} ${"p50 ms".padStart(9)} ${"p95 ms".padStart(9)} ${"p99 ms".padStart(9)} ${"max ms".padStart(9)}\n`;
  for (const s of r.stages) {
    out += `${s.name.padEnd(26)} ${String(s.count).padStart(5)} ${s.mean_ms.toFixed(3).padStart(9)} ${s.p50_ms.toFixed(3).padStart(9)} ${s.p95_ms.toFixed(3).padStart(9)} ${s.p99_ms.toFixed(3).padStart(9)} ${s.max_ms.toFixed(3).padStart(9)}`;
    if (s.over_33ms_share !== undefined) out += `  over 33.3 ms: ${(s.over_33ms_share * 100).toFixed(1)}%`;
    out += "\n";
  }
  if (r.extra.mode && r.extra.mode !== "back-to-back") {
    out += `\nmode=${r.extra.mode} duration=${r.extra.duration_s.toFixed(1)} s dropped frames=${r.extra.dropped_frames}\n`;
    out += `${"minute".padStart(6)} ${"frames".padStart(7)} ${"p50 ms".padStart(9)} ${"p95 ms".padStart(9)} ${"p99 ms".padStart(9)} ${"max ms".padStart(9)}  over 33.3 ms\n`;
    for (const row of r.extra.per_minute) {
      out += `${String(row.minute).padStart(6)} ${String(row.frames).padStart(7)} ${row.p50.toFixed(3).padStart(9)} ${row.p95.toFixed(3).padStart(9)} ${row.p99.toFixed(3).padStart(9)} ${row.max.toFixed(3).padStart(9)}  ${(row.over_33ms_share * 100).toFixed(1)}%\n`;
    }
  }
  if (r.extra.camera) out += `camera: ${JSON.stringify(r.extra.camera)}\n`;
  out += `\nwasm linear memory: ${r.extra.wasm_memory_start} -> ${r.extra.wasm_memory_end} bytes\n`;
  return out;
}

function finish(report) {
  console.log(table(report));
  const json = opt("json", undefined);
  if (json) {
    mkdirSync(dirname(json), { recursive: true });
    writeFileSync(json, JSON.stringify(report, null, 2));
  }
  // Steady state: wasm linear memory must not grow after warm-up.
  if (report.extra.wasm_memory_end > report.extra.wasm_memory_start) {
    console.error(`FAIL: wasm linear memory grew from ${report.extra.wasm_memory_start} to ${report.extra.wasm_memory_end} bytes after warm-up`);
    return 1;
  }
  return 0;
}

let code = 0;
if (deviceMode) {
  // The page on the phone posts its report back (`adb reverse tcp:N tcp:N` makes the phone's
  // localhost:N reach this server, and localhost is a secure context, so COOP/COEP hold).
  let got;
  const reported = new Promise((resolve) => (got = resolve));
  const server = await serve(port, {}, got);
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(options)) if (v !== undefined && v !== null && v !== false) q.set(k, v === true ? "1" : String(v));
  console.error(`Open this on the phone and tap Start:\n  http://localhost:${port}/bench.html?${q}\nWaiting for the report…`);
  const report = await reported;
  if (report.error) {
    console.error(`FAIL: the page reported an error:\n${report.error}`);
    code = 1;
  } else {
    code = finish(report);
  }
  server.close();
  process.exit(code);
}

const server = await serve(port);
let browser;
let url = `http://127.0.0.1:${server.address().port}/bench.html`;
if (android) {
  // Experimental: Playwright's Android support. `adb reverse` lets the phone reach this server
  // as localhost:PORT, a secure context.
  const { _android } = await import("playwright-core");
  const [device] = await _android.devices();
  if (!device) {
    console.error("no Android device: check `adb devices`");
    process.exit(2);
  }
  await promisify(execFile)("adb", ["-s", device.serial(), "reverse", `tcp:${port}`, `tcp:${port}`]);
  browser = await device.launchBrowser({ pkg: opt("android-package", "com.android.chrome") });
  url = `http://localhost:${port}/bench.html`;
} else {
  browser = await launch();
}
try {
  const page = await browser.newPage();
  page.on("console", (m) => console.error(`[page] ${m.text()}`));
  await page.goto(url);
  await page.waitForFunction("window.ready === true");
  const report = await page.evaluate((o) => window.runBench(o), options);
  code = finish(report);
} finally {
  await browser.close();
  server.close();
}
process.exit(code);
