// Regenerates the storefront screenshots from the real renderer.
//
// Not a mock-up: it builds `soko-storefront`, runs it, and photographs what it prints. If the
// crates change what an offer says, this picture changes with them — which is the only way a
// screenshot in a README stays true.
//
// Usage: node tools/screenshots.mjs      (needs Chrome; CHROME_PATH overrides)

import puppeteer from "../../tract/build/node_modules/puppeteer-core/lib/esm/puppeteer/puppeteer-core.js";
import { execFileSync } from "node:child_process";
import { writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "docs/screenshots");

console.log("building soko-storefront…");
const html = execFileSync(
  "cargo",
  ["run", "-q", "-p", "soko-gateway", "--bin", "soko-storefront"],
  { cwd: root, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 }
);

const tmp = join(mkdtempSync(join(tmpdir(), "soko-shot-")), "store.html");
writeFileSync(tmp, html);

const chrome =
  process.env.CHROME_PATH ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const browser = await puppeteer.launch({ executablePath: chrome, headless: "new" });

const shots = [
  { name: "storefront", width: 1280, height: 1000, full: true },
  { name: "storefront-mobile", width: 390, height: 900, full: true },
];

for (const s of shots) {
  const page = await browser.newPage();
  await page.setViewport({ width: s.width, height: s.height, deviceScaleFactor: 2 });
  await page.goto(`file://${tmp}`, { waitUntil: "networkidle0" });
  const path = join(out, `${s.name}.png`);
  await page.screenshot({ path, fullPage: s.full });
  console.log("wrote", path);
  await page.close();
}
await browser.close();
