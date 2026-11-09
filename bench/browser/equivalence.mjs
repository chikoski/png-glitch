// npm run equivalence [-- --browser-channel chrome]
//
// For a fixed corpus of glitched models, compares the component's render-rgba() byte for byte
// with what Chrome's own PNG decoder reads from snapshot(): createImageBitmap + canvas for
// opaque images, WebCodecs ImageDecoder for images with alpha (a canvas premultiplies alpha, so
// it is lossy for alpha < 255) and an ImageBitmap read back through WebGL without
// premultiplying (byte exact). ImageDecoder is also tried: Chrome returns premultiplied alpha
// from it whatever `premultiplyAlpha` says, which is reported as `premultiplied`, not as a
// failure. Colour management is switched off. Exits 1 on any other difference.
import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { serve } from "./server.mjs";

const argv = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : fallback;
};
const channel = opt("browser-channel", undefined);
const executablePath = opt("browser-path", process.env.CHROME_PATH ?? (existsSync("/opt/pw-browsers/chromium") ? "/opt/pw-browsers/chromium" : undefined));

const server = await serve();
const browser = await chromium.launch({
  headless: true,
  ...(channel ? { channel } : executablePath ? { executablePath } : {}),
  args: ["--no-sandbox"],
});
let code = 0;
try {
  const page = await browser.newPage();
  page.on("pageerror", (e) => console.error(`[page] ${e.message}`));
  await page.goto(`http://127.0.0.1:${server.address().port}/equivalence.html`);
  await page.waitForFunction("window.ready === true");
  const { userAgent, results } = await page.evaluate(() => window.runEquivalence());
  console.log(userAgent);
  for (const r of results) console.log(`${r.status.padEnd(8)} ${r.via.padEnd(12)} ${r.label}${r.detail ? `  ${r.detail}` : ""}`);
  const count = (s) => results.filter((r) => r.status === s).length;
  console.log(
    `\n${count("equal")} equal, ${count("premultiplied")} premultiplied by ImageDecoder (documented), ` +
      `${count("differs")} differ, ${count("skipped")} skipped of ${results.length}`,
  );
  if (count("differs") > 0 || count("equal") === 0) code = 1;
} finally {
  await browser.close();
  server.close();
}
process.exit(code);
