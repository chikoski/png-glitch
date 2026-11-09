# png-glitch browser benchmark

Headless Chrome harness for the wasm component (`crates/png-glitch-component`): the `session` live loop in a Web Worker, per-stage timings, and a byte-exact decoder-equivalence check. See the **Browser benchmark** section of [DEVELOPER_GUIDE.md](../../DEVELOPER_GUIDE.md#browser-benchmark-benchbrowser).

```sh
npm ci
npm run build
npm run bench -- --size 1080p --frames 300
npm run equivalence
npm test
```
