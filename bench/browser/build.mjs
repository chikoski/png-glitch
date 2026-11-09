// Builds the component (wasm32-wasip2, simd128) and transpiles it with jco into ./dist.
//
//   node build.mjs [--repo DIR] [--no-cargo]
//
// --repo builds another checkout (the CI job uses it for the base commit). The transpiled
// module is patched in two ways: the bare `@bytecodealliance/preview2-shim/*` imports become
// relative paths (module workers have no import maps), and `__memoryBytes()` is exported so the
// harness can assert that wasm linear memory does not grow in steady state.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const flag = (name) => args.includes(`--${name}`);
const value = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const repo = resolve(value("repo", join(here, "../..")));
const out = resolve(value("out", join(here, "dist")));

if (!flag("no-cargo")) {
  execFileSync(
    "cargo",
    ["build", "-p", "png-glitch-component", "--target", "wasm32-wasip2", "--release"],
    {
      cwd: repo,
      stdio: "inherit",
      env: { ...process.env, RUSTFLAGS: `${process.env.RUSTFLAGS ?? ""} -C target-feature=+simd128`.trim() },
    },
  );
}
const wasm = join(repo, "target/wasm32-wasip2/release/png_glitch_component.wasm");
if (!existsSync(wasm)) throw new Error(`${wasm} not found`);

rmSync(out, { recursive: true, force: true });
const jco = join(here, "node_modules/.bin/jco");
execFileSync(jco, ["transpile", wasm, "-o", out, "--name", "component"], { stdio: "inherit" });

const file = join(out, "component.js");
let code = readFileSync(file, "utf8");
const shim = "../node_modules/@bytecodealliance/preview2-shim/dist/browser";
code = code.replace(/"@bytecodealliance\/preview2-shim\/([a-z-]+)"/g, `"${shim}/$1.js"`);
if (!/^let memory0;$/m.test(code)) throw new Error("jco output changed: `let memory0;` not found");
code += "\nexport const __memoryBytes = () => (memory0 ? memory0.buffer.byteLength : 0);\n";
writeFileSync(file, code);
console.log(`component transpiled to ${out}`);
