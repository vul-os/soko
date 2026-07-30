// Renders the architecture diagrams to PNG for the README, the landing page and the docs.
//
// There is no product to screenshot yet — Soko is pre-alpha with no UI — and a mocked-up product
// shot would misrepresent how far along this is. These are the real artefacts: the same mermaid
// sources the docs render, exported so the visual slots carry something true.
//
// Usage: npm install && node tools/diagrams.mjs   (needs Chrome; set CHROME_PATH to override)
//
// puppeteer-core is soko's own devDependency (package.json at the repo root) — this
// used to reach across into a sibling ../../tract checkout's node_modules, which
// silently stopped resolving once TRACT folded into kotva as a profile (see
// CHANGELOG.md). Fixed here rather than repointed at another sibling repo: soko
// must not depend on any other product repo, by path or otherwise.

import puppeteer from "puppeteer-core";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");
const mermaidJs = readFileSync(
  join(root, "site/assets/vendor/mermaid.min.js"),
  "utf8"
);

const BG = "#0E0E10";
const DIAGRAMS = {
  "cart-flow": `flowchart LR
  subgraph SELLERS["Sellers — each sovereign"]
    direction TB
    SA["Seller A<br/><i>signed feed</i>"]
    SB["Seller B<br/><i>signed feed</i>"]
    SC["Seller C<br/><i>signed feed</i>"]
  end
  subgraph DISC["Discovery"]
    IDX["Any index<br/><i>derived · rebuildable</i><br/><i>authoritative for nothing</i>"]
  end
  subgraph BUYER["Buyer"]
    B["<b>Buyer node</b><br/>one cart across all three<br/>routing computed here<br/>sealed order back to each seller"]
  end
  SA --> IDX
  SB --> IDX
  SC --> IDX
  IDX --> B`,

  "substrate": `flowchart TD
  subgraph S["DMTAP substrate — the narrow waist"]
    I["① Identity"]; F["② Feeds &amp; Blobs"]; Y["③ Sync"]; R["④ Infra Roles"]; W["⑤ Wake"]
  end
  S --> T["<b>TRACT</b> — the commerce spine<br/>catalogue · offer · cart · order<br/>delivery · settlement · trust"]
  T --> K["<b>Soko</b><br/><i>this implementation</i>"]
  S --> M["DMTAP-mail"]`,

  "checkout": `sequenceDiagram
  autonumber
  participant B as Buyer node
  participant S1 as Seller A
  participant S2 as Seller B
  participant G as Gateway (only if escrow chosen)
  B->>B: compute routing + totals locally
  B->>S1: sealed order (A's lines only)
  B->>S2: sealed order (B's lines only)
  S1-->>B: accept / decline / counter
  S2-->>B: accept / decline / counter
  opt escrow agreed by both parties
    B->>G: pay
  end
  S1->>B: fulfil
  B->>G: confirm receipt
  G->>S1: release`,

  "routing": `flowchart LR
  subgraph D["Option 1 — direct · N parcels"]
    direction LR
    A1["Seller A"] --> BX1["Buyer"]
    A2["Seller B"] --> BX1
  end
  subgraph H["Option 2 — hub near buyer · 1 parcel"]
    direction LR
    B1["Seller A"] --> HUB["Distributor<br/><i>holds · consolidates</i>"]
    B2["Seller B"] --> HUB
    HUB --> BX2["Buyer"]
  end
  NOTE["<b>Compared locally, on the buyer's node</b><br/>&nbsp;<br/>Σ legs&nbsp;&nbsp;+&nbsp;&nbsp;storage × wait_days&nbsp;&nbsp;+&nbsp;&nbsp;handling<br/>&nbsp;<br/><i>no quote API&nbsp;·&nbsp;no broker&nbsp;·&nbsp;nobody learns the cart</i>"]`,

  "axes": `flowchart LR
  O["<b>Offer</b>"] --> I["<b>Item</b><br/>product · variant · service<br/>right · capacity"]
  O --> A["<b>Availability</b><br/>count · slots · capacity<br/>unlimited · made-to-order"]
  O --> F["<b>Fulfilment</b><br/>ship · collect · digital<br/>perform-at · perform-remote<br/>access · return-required"]
  O --> C["<b>Consideration</b><br/>fixed · tiered · recurring<br/>metered · deposit · quote"]
  F -.->|"also derives"| P["place of supply<br/><i>the tax anchor</i>"]`,
};

const chrome =
  process.env.CHROME_PATH ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const browser = await puppeteer.launch({ executablePath: chrome, headless: "new" });

for (const [name, src] of Object.entries(DIAGRAMS)) {
  const page = await browser.newPage();
  await page.setViewport({ width: 1400, height: 900, deviceScaleFactor: 2 });
  await page.setContent(`<html><body style="margin:0;background:${BG}">
    <div id="d" style="display:inline-block;padding:36px"><pre class="mermaid">${src}</pre></div>
    <script>${mermaidJs}</script>
    <script>
      mermaid.initialize({startOnLoad:false, theme:'dark', securityLevel:'loose',
        themeVariables:{
          fontFamily:'-apple-system,BlinkMacSystemFont,Segoe UI,Inter,Roboto,Helvetica,Arial,sans-serif',
          fontSize:'15px',
          background:'${BG}', primaryColor:'#18181B', primaryTextColor:'#FAFAFA',
          primaryBorderColor:'#16D97F', lineColor:'#16D97F',
          secondaryColor:'#131316', tertiaryColor:'#131316',
          actorBkg:'#18181B', actorBorder:'#16D97F', actorTextColor:'#FAFAFA',
          signalColor:'#16D97F', signalTextColor:'#A1A1AA',
          clusterBkg:'#131316', clusterBorder:'#2A2A2E',
          edgeLabelBackground:'#0E0E10', labelBackground:'#0E0E10', labelBoxBorderColor:'#16D97F',
          noteBkgColor:'#131316', noteTextColor:'#FAFAFA', noteBorderColor:'#16D97F'
        }});
      window.__done = mermaid.run({querySelector:'pre.mermaid'}).then(()=>true);
    </script></body></html>`);
  await page.waitForFunction(() => window.__done !== undefined, { timeout: 15000 });
  await page.evaluate(() => window.__done);
  await new Promise((r) => setTimeout(r, 250));
  const el = await page.$("#d");
  const out = join(root, "docs/diagrams", `${name}.png`);
  await el.screenshot({ path: out });
  console.log("wrote", out);
  await page.close();
}
await browser.close();
