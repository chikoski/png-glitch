# png-glitch browser benchmark

Headless Chrome harness for the wasm component (`crates/png-glitch-component`): the `session` live loop in a Web Worker, per-stage timings, and a byte-exact decoder-equivalence check. See the **Browser benchmark** section of [DEVELOPER_GUIDE.md](../../DEVELOPER_GUIDE.md#browser-benchmark-benchbrowser).

```sh
npm ci
npm run build
npm run bench -- --size 1080p --frames 300
npm run equivalence
npm test
```

## Reference device: Chrome on Android (#222)

The reference device is the Pixel 11 Pro Fold with Chrome for Android, at 30 fps (#99). The phone opens the page through `adb reverse`, so it is `http://localhost:PORT`, a secure context: COOP/COEP, `crossOriginIsolated` and the Worker work. The page keeps the screen on with a wake lock.

```sh
adb devices                       # the phone is listed (USB debugging on)
adb reverse tcp:8123 tcp:8123
# 1. --device: the page on the phone posts its report back (works with any Chrome)
npm run bench -- --device --port 8123 --size 1080p --duration 600 --note "unfolded, battery saver off, plugged in" --json out/pixel.json
#    open the printed URL in Chrome on the phone and tap Start
# 2. --android: Playwright drives Chrome on the phone (experimental, needs adb on the PATH)
npm run bench -- --android --port 8123 --size 1080p --duration 600 --json out/pixel.json
```

- `--duration S` is the **sustained mode**: frame k is due at `k / 30 s`; the latest due frame runs and frames we are behind on are **dropped, not queued**. The report adds `extra.dropped_frames` and `extra.per_minute` (`p50`, `p95`, `p99`, `max` in ms and `over_33ms_share` of `frame` for each minute, so throttling shows as a trend). `--frames` (back to back) stays the default. Samples are kept in preallocated `Float64Array`s. In this mode `snapshot` is not sampled, and `transfer-in` / `draw-out` / `copy-in-floor` / `boundary-in` are sampled every 10th frame only when more than 12 ms of the frame budget is left.
- `--camera` takes frames from `getUserMedia({ video: { width: 1920, height: 1080, frameRate: 30 } })` and times the step "frame in" as `browser/camera-in` (outside `browser/frame`). It needs the camera permission and a user gesture: in `--device` mode the Start button is the gesture. The camera sets the pace, so the run is not reproducible; the synthetic generator stays the default. `extra.camera` holds `track.getSettings()` (what the camera actually delivered), the path used (`copyTo-rgba` or `canvas`) and the number of frames delivered. On desktop headless Chrome `--camera` uses a fake camera.
- The device state is in `extra`: `user_agent`, `screen` (size, `device_pixel_ratio`, orientation), `hardware_concurrency`, `device_memory`, `cross_origin_isolated`, `battery` (`charging`, `level`). Folded or unfolded and battery saver cannot be detected: say it with `--note`, copied to `extra.note`. The canvas size changes when the phone is unfolded.
- The JSON has the same format as the desktop runs; `png-glitch-bench compare` reads it.
- Then post `browser/frame` (p50, p99, share over 33.3 ms) and the `per_minute` rows of a 10-minute run in #99.
