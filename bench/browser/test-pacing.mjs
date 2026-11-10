// Unit tests of the pure parts of the sustained mode: pacing / dropping and the per-minute summary.
import assert from "node:assert/strict";
import { BUDGET_MS, MINUTE_MS, nextFrame, perMinute, simulatePacing, summarize } from "./pacing.mjs";

const tests = [];
const test = (name, fn) => tests.push([name, fn]);

test("first frame runs at once", () => {
  assert.deepEqual(nextFrame(-1, 0), { index: 0, dropped: 0, waitMs: 0 });
});

test("a fast frame waits for the next due time", () => {
  const n = nextFrame(0, 10);
  assert.equal(n.index, 1);
  assert.equal(n.dropped, 0);
  assert.ok(Math.abs(n.waitMs - (BUDGET_MS - 10)) < 1e-9);
});

test("a late frame runs at once without dropping", () => {
  assert.deepEqual(nextFrame(0, 50), { index: 1, dropped: 0, waitMs: 0 });
});

test("frames we are behind on are dropped, not queued", () => {
  // 70 ms after the start: frames 0 ran; frame 1 (33 ms) and 2 (66 ms) are due; run the latest.
  assert.deepEqual(nextFrame(0, 70), { index: 2, dropped: 1, waitMs: 0 });
});

test("fast processing drops nothing", () => {
  const r = simulatePacing(() => 5, 1000);
  assert.equal(r.dropped, 0);
  assert.equal(r.ran.length, 30);
  assert.deepEqual(r.ran.slice(0, 3), [0, 1, 2]);
});

test("every frame 50 ms: frames are dropped and indices only grow", () => {
  const r = simulatePacing(() => 50, 1000);
  assert.ok(r.dropped > 0);
  // frames dropped after the last one that ran are not counted
  assert.ok(r.ran.length + r.dropped <= 30 && r.ran.length + r.dropped >= 28);
  for (let i = 1; i < r.ran.length; i++) assert.ok(r.ran[i] > r.ran[i - 1]);
});

test("one slow frame drops only the frames it overran", () => {
  const r = simulatePacing((i) => (i === 3 ? 100 : 5), 1000);
  assert.equal(r.dropped, 2); // 100 ms covers the due times of two more frames
  assert.equal(r.ran.length + r.dropped, 30);
});

test("summarize matches the nearest-rank percentiles", () => {
  const s = summarize([5, 1, 3, 2, 4]);
  assert.equal(s.count, 5);
  assert.equal(s.p50_ms, 3);
  assert.equal(s.max_ms, 5);
  assert.equal(s.mean_ms, 3);
  // only the first `length` entries of a preallocated array count
  assert.equal(summarize(new Float64Array([1, 2, 3, 0, 0]), 3).max_ms, 3);
});

test("perMinute groups by minute and counts frames over the budget", () => {
  const starts = [0, 1000, MINUTE_MS, MINUTE_MS + 1000, MINUTE_MS + 2000, 2 * MINUTE_MS + 5];
  const dur = [10, 40, 20, 20, 50, 30];
  const rows = perMinute(starts, dur, 6);
  assert.deepEqual(rows.map((r) => r.minute), [0, 1, 2]);
  assert.deepEqual(rows.map((r) => r.frames), [2, 3, 1]);
  assert.equal(rows[0].max, 40);
  assert.equal(rows[0].over_33ms_share, 0.5);
  assert.equal(rows[1].p50, 20);
  assert.ok(Math.abs(rows[1].over_33ms_share - 1 / 3) < 1e-9);
  assert.equal(rows[2].p99, 30);
});

test("perMinute ignores the unused tail of a preallocated array", () => {
  const rows = perMinute(new Float64Array([0, 1, 0, 0]), new Float64Array([1, 2, 99, 99]), 2);
  assert.equal(rows.length, 1);
  assert.equal(rows[0].frames, 2);
  assert.equal(rows[0].max, 2);
});

let failed = false;
for (const [name, fn] of tests) {
  try {
    fn();
    console.log(`ok   ${name}`);
  } catch (e) {
    failed = true;
    console.log(`FAIL ${name}\n${e.message}`);
  }
}
process.exit(failed ? 1 : 0);
