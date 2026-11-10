// Pure parts of the sustained mode (#222), shared by the worker and the tests.
export const BUDGET_MS = 1000 / 30;
export const MINUTE_MS = 60_000;

// Statistics of the first `length` entries of `samples` (an array or a typed array).
export function summarize(samples, length = samples.length) {
  const s = Array.from(samples.subarray ? samples.subarray(0, length) : samples.slice(0, length)).sort((a, b) => a - b);
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

// Frame k is due at start + k * budget. After frame `lastIndex` was processed, `elapsedMs`
// after the start: the next frame to run is the latest one that is due, never an older one
// (frames we are behind on are dropped, not queued, as #99 requires). `waitMs` is how long to
// wait until it is due (0 when it already is).
export function nextFrame(lastIndex, elapsedMs, budgetMs = BUDGET_MS) {
  const due = Math.floor(elapsedMs / budgetMs + 1e-9);
  const index = Math.max(lastIndex + 1, due);
  return { index, dropped: index - lastIndex - 1, waitMs: Math.max(0, index * budgetMs - elapsedMs) };
}

// Simulates a paced run with a virtual clock. `cost(i)` is the processing time of the i-th
// processed frame. Returns the indices of the frames that ran, their start times, and the
// number of dropped frames. A frame whose due time is after the end is not run.
export function simulatePacing(cost, durationMs, budgetMs = BUDGET_MS) {
  const ran = [];
  const starts = [];
  let dropped = 0;
  let last = -1;
  let clock = 0;
  for (;;) {
    const n = nextFrame(last, clock, budgetMs);
    const startAt = Math.max(clock, n.index * budgetMs);
    if (n.index * budgetMs >= durationMs) break;
    dropped += n.dropped;
    ran.push(n.index);
    starts.push(startAt);
    clock = startAt + cost(ran.length - 1);
    last = n.index;
  }
  return { ran, starts, dropped };
}

// The latency of `frame` per minute of the run: `startsMs[i]` is when frame i started, counted
// from the start of the timed run, and `durationsMs[i]` how long it took. A trend in these
// rows is thermal throttling. Times are in ms; `over_33ms_share` is a fraction.
export function perMinute(startsMs, durationsMs, length = durationsMs.length, budgetMs = BUDGET_MS) {
  const groups = new Map();
  for (let i = 0; i < length; i++) {
    const m = Math.floor(startsMs[i] / MINUTE_MS);
    if (!groups.has(m)) groups.set(m, []);
    groups.get(m).push(durationsMs[i]);
  }
  return [...groups.keys()]
    .sort((a, b) => a - b)
    .map((minute) => {
      const d = groups.get(minute);
      const s = summarize(d);
      return {
        minute,
        frames: d.length,
        p50: s.p50_ms,
        p95: s.p95_ms,
        p99: s.p99_ms,
        max: s.max_ms,
        over_33ms_share: d.filter((x) => x > budgetMs).length / d.length,
      };
    });
}
