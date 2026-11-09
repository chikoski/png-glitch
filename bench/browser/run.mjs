// npm run bench -- --size 1080p --frames 300 [--warmup 10] [--pool 8] [--recipe swap]
//                  [--json out/browser.json] [--browser-channel chrome] [--commit SHA]
//
// Runs the component's live loop in a Web Worker of headless Chrome and prints a table. The
// JSON has the schema of `png-glitch-bench --json`.
import { chromium } from "playwright-core";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { dirname } from "node:path";
import { serve } from "./server.mjs";

const argv = process.argv.slice(2);
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
    args: ["--no-sandbox", "--enable-features=SharedArrayBuffer", "--enable-precise-memory-info"],
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
};

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
  out += `\nwasm linear memory: ${r.extra.wasm_memory_start} -> ${r.extra.wasm_memory_end} bytes\n`;
  return out;
}

const server = await serve();
const browser = await launch();
let code = 0;
try {
  const page = await browser.newPage();
  page.on("console", (m) => console.error(`[page] ${m.text()}`));
  await page.goto(`http://127.0.0.1:${server.address().port}/bench.html`);
  await page.waitForFunction("window.ready === true");
  const report = await page.evaluate((o) => window.runBench(o), options);
  console.log(table(report));
  const json = opt("json", undefined);
  if (json) {
    mkdirSync(dirname(json), { recursive: true });
    writeFileSync(json, JSON.stringify(report, null, 2));
  }
  // Steady state: wasm linear memory must not grow after warm-up.
  if (report.extra.wasm_memory_end > report.extra.wasm_memory_start) {
    console.error(`FAIL: wasm linear memory grew from ${report.extra.wasm_memory_start} to ${report.extra.wasm_memory_end} bytes after warm-up`);
    code = 1;
  }
} finally {
  await browser.close();
  server.close();
}
process.exit(code);
