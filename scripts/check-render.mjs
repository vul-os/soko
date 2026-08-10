/**
 * site/ render gate — loads site/index.html and site/docs.html in a real
 * browser at every width the pages claim to support, in both colour schemes,
 * and fails on the defects that reading the HTML cannot see.
 *
 * Why a browser: every defect this catches is invisible in the source. A label
 * declared at 15px and drawn at 3.9px because its SVG is scaled to fit; an
 * inline <code> run pushing the page sideways at 320 while body{overflow-x:clip}
 * hides the evidence; an image stretched off its aspect ratio; a cross-reference
 * to a heading id that no longer exists; a font quietly fetched off-box.
 *
 *   node scripts/check-render.mjs              # serve site/ and measure it
 *   node scripts/check-render.mjs --selftest   # break each check on purpose
 *
 * Every check reports what it MEASURED, not just pass/fail, so a green run is
 * evidence rather than an assertion.
 */

import { createServer } from 'http';
import { readFile, readdir } from 'fs/promises';
import { existsSync } from 'fs';
import { createRequire } from 'module';
import { resolve, dirname, extname, join, normalize } from 'path';
import { fileURLToPath, pathToFileURL } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(__dirname, '..');
const SITE = resolve(REPO, 'site');

// ---------------------------------------------------------------------------
// What this gate believes it is measuring.
//
// A concurrent agent's stray `python3 -m http.server` elsewhere in this fleet
// once served a DIFFERENT product's site on the port a gate assumed, and 410
// width×scheme combinations came back green with nothing in the output looking
// wrong. This gate binds an EPHEMERAL port (port 0, so the kernel hands out one
// nothing is already squatting on) and still refuses to trust a measurement
// until the served <title> proves the document is this product's.
// ---------------------------------------------------------------------------
const TITLE_MUST_MATCH = {
  'index.html': /^Soko\b/,
  'docs.html': /^Soko docs\b/,
};

const PAGES = ['index.html', 'docs.html'];

// docs.html is a hash router: one shell that fetches site/docs/<slug>.md into
// the content column. Measuring `docs.html` alone therefore measures exactly ONE
// chapter and the rest ride along untested. The full width×scheme matrix runs
// against the landing, the default chapter and the heaviest chapter; a separate
// narrow/wide sweep then covers every remaining chapter.
const MATRIX_TARGETS = ['index.html', 'docs.html', 'docs.html#diagrams'];

const VIEWPORTS = [
  { w: 1920, h: 1080, label: 'desktop-xl' },
  { w: 1440, h: 900, label: 'desktop' },
  { w: 1280, h: 800, label: 'laptop' },
  { w: 1024, h: 768, label: 'tablet-landscape' },
  { w: 768, h: 1024, label: 'tablet' },
  { w: 430, h: 932, label: 'phone-large' },
  { w: 390, h: 844, label: 'phone' },
  { w: 320, h: 720, label: 'phone-min' },
];

// ---------------------------------------------------------------------------
// Playwright, from wherever it lives on this box.
// A bare `import 'playwright'` resolves relative to THIS file's directory, so
// it only works when scripts/ has its own node_modules; walk the fleet for a
// checkout that has one otherwise.
// ---------------------------------------------------------------------------
async function loadPlaywright() {
  const tried = [];
  try { return await import('playwright'); } catch { tried.push('<bare specifier>'); }

  const candidates = [];
  if (process.env.PLAYWRIGHT_NODE_MODULES) candidates.push(process.env.PLAYWRIGHT_NODE_MODULES);
  candidates.push(join(__dirname, 'node_modules'));
  const seen = new Set();
  for (const up of ['..', '../..']) {
    const root = resolve(REPO, up);
    if (seen.has(root)) continue;
    seen.add(root);
    let entries = [];
    try { entries = await readdir(root); } catch { continue; }
    for (const e of entries) {
      if (e.startsWith('.')) continue;
      candidates.push(join(root, e, 'node_modules'));
      candidates.push(join(root, e, 'web', 'node_modules'));
      candidates.push(join(root, e, 'scripts', 'node_modules'));
      for (const app of ['desktop', 'web', 'site']) {
        candidates.push(join(root, e, 'apps', app, 'node_modules'));
      }
    }
  }

  for (const root of candidates) {
    if (!root || !existsSync(join(root, 'playwright'))) continue;
    try {
      const req = createRequire(join(root, '__resolve__.js'));
      const entry = req.resolve('playwright');
      const raw = await import(pathToFileURL(entry).href);
      // Playwright is CommonJS. Importing it by PATH skips Node's named-export
      // detection, so the launchers arrive under `default` rather than as named
      // exports — a different shape from a bare `import('playwright')`.
      const mod = raw?.chromium ? raw : raw?.default;
      // Resolvable is not the same as usable: a stub or a partial install
      // resolves fine and then dies three calls later.
      if (!mod?.chromium?.launch) { tried.push(`${root} (no chromium launcher)`); continue; }
      console.log(`check-render: using playwright from ${root}`);
      return mod;
    } catch (e) { tried.push(`${root} (${e.code || e.message})`); }
  }

  console.error('check-render: could not load playwright.\n  tried: ' + tried.join('\n         ') +
    '\n  set PLAYWRIGHT_NODE_MODULES=/path/to/node_modules');
  process.exit(2);
}

const MIME = {
  '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'text/javascript',
  '.mjs': 'text/javascript', '.json': 'application/json', '.md': 'text/markdown; charset=utf-8',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2', '.woff': 'font/woff', '.txt': 'text/plain; charset=utf-8',
  '.ico': 'image/x-icon', '.webp': 'image/webp', '.xml': 'application/xml',
  '.webmanifest': 'application/manifest+json', '.avif': 'image/avif', '.gif': 'image/gif',
};

function serve(root) {
  return new Promise(ok => {
    const s = createServer(async (req, res) => {
      const rel = normalize(decodeURIComponent(req.url.split('?')[0])).replace(/^(\.\.[/\\])+/, '');
      let file = join(root, rel);
      if (!extname(file)) file = join(file, 'index.html');
      try {
        const body = await readFile(file);
        res.writeHead(200, { 'content-type': MIME[extname(file)] || 'application/octet-stream' });
        res.end(body);
      } catch {
        res.writeHead(404).end('not found');
      }
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------
const findings = [];
const notes = [];
// Totals a green run prints, so "no findings" is backed by what was seen rather
// than being an unexamined silence.
const stats = { chapterLoads: 0, chapterChars: 0, mermaidPainted: 0 };
const fail = (check, where, detail) => findings.push({ check, where, detail });
const note = (s) => notes.push(s);

// ---------------------------------------------------------------------------
// The in-page measurement pass.
// ---------------------------------------------------------------------------
async function inspect(page) {
  return page.evaluate(async () => {
    const out = {
      title: document.title, overflow: null, imgs: [], smallText: [], textScanned: 0,
      deadAnchors: [], anchorsScanned: 0,
    };

    // 1 · horizontal overflow, measured GEOMETRICALLY.
    //
    // `documentElement.scrollWidth - clientWidth <= 1` is NOT usable. Measured
    // here, 2400px child in a 1440 viewport: under html,body{overflow-x:clip}
    // documentElement.scrollWidth reports 1440, and under
    // html,body{overflow-x:hidden} it reports 1440 while body.scrollWidth reads
    // 2400 — the assertion passes vacuously while the page is genuinely cut
    // off. A clipping wrapper does the same. So walk the elements instead: an element is a defect if its box
    // crosses the viewport edge and NO ancestor between it and <body> clips or
    // scrolls horizontally. Stopping the ancestor walk BEFORE body is the whole
    // point — body's own clip is what hides the bug, not a licence for it. A
    // wide <pre> or table inside its own overflow-x:auto box is deliberate
    // design and is correctly ignored.
    const de = document.documentElement;
    const bleed = [];
    const clipped = [];
    document.querySelectorAll('body *').forEach(el => {
      const cs = getComputedStyle(el);
      if (cs.position === 'fixed') return;
      if (cs.visibility === 'hidden' || cs.display === 'none') return;
      const r = el.getBoundingClientRect();
      if (r.width <= 0 || r.height <= 0) return;
      let p = el.parentElement, contained = false;
      while (p && p !== document.body) {
        const pcs = getComputedStyle(p);
        if (/auto|scroll|hidden|clip/.test(pcs.overflowX)) {
          contained = true;
          // …but "contained" is not automatically "fine". auto/scroll means the
          // reader can pan to the rest; hidden/clip means the rest is gone. This
          // gate shipped a card with overflow:hidden around a diagram wider than
          // the card, and the overflow scan called it contained while a third of
          // the drawing was unreachable — caught by eye, not by the check, which
          // is the exact failure mode this file exists to end.
          if (!/auto|scroll/.test(pcs.overflowX)) {
            const pr = p.getBoundingClientRect();
            const lost = Math.round(Math.max(0, r.right - pr.right) + Math.max(0, pr.left - r.left));
            const carries = el.tagName === 'IMG' || el.tagName === 'SVG' ||
              [...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim().length > 1);
            if (lost > 2 && carries) {
              clipped.push({
                tag: el.tagName, cls: String(el.className.baseVal ?? el.className).trim().slice(0, 40),
                lost, by: p.tagName + (p.className ? '.' + String(p.className.baseVal ?? p.className).trim().slice(0, 30) : ''),
                overflow: pcs.overflowX,
              });
            }
          }
          break;
        }
        p = p.parentElement;
      }
      if (contained) return;
      // Content parked wholly off-screen left (a skip link at -9999px) adds no
      // horizontal scroll; only a right-edge crossing does, plus anything
      // straddling the left edge and therefore partly unreachable.
      if (r.right <= 0) return;
      if (r.right > window.innerWidth + 1 || r.left < -1) {
        bleed.push({
          tag: el.tagName, cls: String(el.className.baseVal ?? el.className).trim().slice(0, 50),
          left: Math.round(r.left), right: Math.round(r.right),
        });
      }
    });
    out.overflow = {
      docW: de.scrollWidth, bodyW: document.body.scrollWidth,
      winW: window.innerWidth, bleed: bleed.slice(0, 8), clipped: clipped.slice(0, 8),
    };

    // 2 · EFFECTIVE text size, i.e. after every transform between the glyph and
    // the screen — not the declared font-size property.
    //
    // This is the check that catches the defect class a property read cannot:
    // elsewhere in this fleet a mermaid config declared fontSize:'15px' while
    // useMaxWidth fitted a 925px drawing into the prose column and scaled the
    // whole SVG — labels included — down to 3.71px. Any gate reading the
    // declared value passes.
    //
    // Two mechanisms scale text and neither shows in font-size:
    //   · CSS transform / zoom on any ancestor;
    //   · an <svg> viewBox mapping user units onto a smaller CSS box.
    // getComputedStyle on an SVG element reports NOMINAL user units, so the
    // second is invisible to it entirely. getScreenCTM is the honest answer and
    // already composes ancestor CSS transforms, so inside an <svg> it is the
    // authority and must not be multiplied by them again.
    const effectiveScale = (el) => {
      const svgHost = el.ownerSVGElement
        || (el.tagName === 'svg' ? el : null)
        || (el.closest ? el.closest('svg') : null);
      if (svgHost && svgHost.getScreenCTM) {
        const m = svgHost.getScreenCTM();
        if (m) return Math.sqrt(Math.abs(m.a * m.d - m.b * m.c)) || 1;
      }
      let s = 1;
      for (let n = el; n && n.nodeType === 1; n = n.parentElement) {
        const cs = getComputedStyle(n);
        if (cs.transform && cs.transform !== 'none') {
          const m = new DOMMatrixReadOnly(cs.transform);
          s *= Math.sqrt(Math.abs(m.a * m.d - m.b * m.c)) || 1;
        }
        const z = parseFloat(cs.zoom);
        if (z && z !== 1 && !Number.isNaN(z)) s *= z;
      }
      return s;
    };

    const FLOOR = 12;
    const TEXTY = new Set(['P', 'LI', 'DD', 'DT', 'TD', 'TH', 'SPAN', 'B', 'I', 'EM', 'STRONG',
      'A', 'CODE', 'FIGCAPTION', 'LABEL', 'SMALL', 'BLOCKQUOTE', 'BUTTON', 'DIV', 'SUMMARY',
      'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'text', 'tspan']);
    // `body *`, not `main *`: elsewhere in this fleet a page with no <main> made
    // the equivalent scan match almost nothing, so the floor passed vacuously
    // and a planted 9px paragraph went straight through a self-test.
    document.querySelectorAll('body *').forEach(el => {
      const tag = el.tagName;
      if (!TEXTY.has(tag) && !TEXTY.has(String(tag).toLowerCase())) return;
      // Only elements holding their OWN text, so a wrapper is not blamed for a
      // child's size and one string is not counted twice.
      const own = [...el.childNodes]
        .filter(n => n.nodeType === 3).map(n => n.textContent.trim()).join('');
      if (own.length < 2) return;
      const cs = getComputedStyle(el);
      if (cs.visibility === 'hidden' || cs.display === 'none') return;
      if (parseFloat(cs.opacity) === 0) return;
      const r = el.getBoundingClientRect();
      if (r.width < 2 || r.height < 2) return;
      out.textScanned++;
      const declared = parseFloat(cs.fontSize);
      const scale = effectiveScale(el);
      const effective = +(declared * scale).toFixed(2);
      if (effective < FLOOR) {
        out.smallText.push({
          tag: String(tag).toLowerCase(), declared, scale: +scale.toFixed(3), effective,
          text: own.slice(0, 40),
        });
      }
    });

    // Generated content is text the reader reads and the element scan above
    // cannot see, because a pseudo-element owns no node.
    document.querySelectorAll('body *').forEach(el => {
      for (const pseudo of ['::before', '::after']) {
        const cs = getComputedStyle(el, pseudo);
        const content = cs.content;
        if (!content || content === 'none' || content === 'normal') continue;
        // Only quoted string content is text; url(), counters and gradients are
        // decoration and have no legibility floor.
        const m = /^"(.*)"$/s.exec(content);
        if (!m || m[1].trim().length < 2) continue;
        if (cs.visibility === 'hidden' || cs.display === 'none') continue;
        const host = getComputedStyle(el);
        if (host.display === 'none' || host.visibility === 'hidden') continue;
        if (el.getBoundingClientRect().width < 2) continue;
        out.textScanned++;
        const scale = effectiveScale(el);
        const declared = parseFloat(cs.fontSize);
        const effective = +(declared * scale).toFixed(2);
        if (effective < FLOOR) {
          out.smallText.push({
            tag: `${el.tagName.toLowerCase()}${pseudo}`, declared,
            scale: +scale.toFixed(3), effective, text: m[1].slice(0, 40),
          });
        }
      }
    });

    // 3 · rendered aspect ratio vs the source's true aspect ratio, and rendered
    // size vs the source's real pixels.
    // naturalWidth is DENSITY-CORRECTED: a candidate chosen through a "2x"
    // descriptor reports half its real pixel width, so re-load currentSrc bare
    // to learn the file's actual size before judging sharpness.
    const trueSize = async (url) => await new Promise(res => {
      const probe = new Image();
      probe.onload = () => res([probe.naturalWidth, probe.naturalHeight]);
      probe.onerror = () => res([0, 0]);
      probe.src = url;
    });
    for (const im of document.querySelectorAll('img')) {
      const r = im.getBoundingClientRect();
      if (r.width < 4 || r.height < 4) continue;
      if (!im.naturalWidth || !im.naturalHeight) continue;
      const url = im.currentSrc || im.src;
      const [px, py] = await trueSize(url);
      // Only object-fit:fill (the default) stretches pixels. cover/contain/none
      // crop or letterbox, so a box ratio unlike the source ratio is intended.
      const fit = getComputedStyle(im).objectFit;
      out.imgs.push({
        file: url.split('/').pop(),
        css: `${Math.round(r.width)}x${Math.round(r.height)}`,
        nat: `${im.naturalWidth}x${im.naturalHeight}`,
        realPx: `${px}x${py}`,
        skewPct: fit === 'fill'
          ? +(Math.abs((r.width / r.height) / (im.naturalWidth / im.naturalHeight) - 1) * 100).toFixed(1)
          : 0,
        upscale: px ? +(r.width * devicePixelRatio / px).toFixed(2) : 0,
      });
    }

    // 4 · same-page fragment links resolve.
    // docs.html is a hash ROUTER, so `#slug` is a route rather than an id and
    // must be judged against the chapter list as well. Rail and on-this-page
    // links that carry their target in a data attribute are checked too.
    const routes = new Set([...document.querySelectorAll('[data-slug]')].map(e => e.dataset.slug));
    document.querySelectorAll('a[href^="#"], a[data-id], a[data-to]').forEach(a => {
      const raw = a.getAttribute('href') || '';
      const ids = [a.dataset.id, a.dataset.to, raw.startsWith('#') ? raw.slice(1) : ''].filter(Boolean);
      for (const id of ids) {
        out.anchorsScanned++;
        if (routes.has(id)) continue;
        if (routes.has(id.split('/')[0])) continue;
        if (document.getElementById(id)) continue;
        if (document.querySelector(`[name="${CSS.escape(id)}"]`)) continue;
        out.deadAnchors.push({ href: '#' + id, text: a.textContent.trim().slice(0, 30) });
      }
    });

    // 5 · the docs shell fetches its prose at runtime. If that fetch fails the
    // page still renders — header, rail, footer, all of it — and every
    // measurement above passes over an empty column. Refuse to call a chapter
    // measured unless its prose is actually there.
    const content = document.querySelector('#content');
    if (content) {
      out.chapter = {
        chars: content.textContent.trim().length,
        headings: content.querySelectorAll('h1,h2,h3').length,
        error: !!content.querySelector('.docs-error, .error'),
        unpaintedMermaid: content.querySelectorAll('code.language-mermaid, pre.mermaid:not([data-processed])').length,
        mermaidSvgs: content.querySelectorAll('.mermaid svg, svg[id^="mermaid"]').length,
      };
    }

    return out;
  });
}

// ---------------------------------------------------------------------------
async function checkPage(browser, base, target, theme, vp) {
  const path = target.split('#')[0];
  const ctx = await browser.newContext({
    viewport: { width: vp.w, height: vp.h }, deviceScaleFactor: 2, colorScheme: theme,
  });
  const page = await ctx.newPage();
  const where = `${target} ${vp.label}(${vp.w}) ${theme}`;

  const httpErrors = [];
  page.on('response', r => { if (r.status() >= 400) httpErrors.push(`${r.status()} ${r.url()}`); });
  page.on('pageerror', e => fail('js-error', where, e.message));

  // Every subresource must come from the page's own origin. These sites claim
  // to be self-contained and air-gappable — the vendored libraries under
  // site/assets/ exist precisely so it is true — and nothing was enforcing it.
  // A <script src="https://cdn…"> or a Google Fonts @import is exactly the kind
  // of thing that arrives in a hurry and then stays.
  const offOrigin = new Set();
  page.on('request', r => {
    const url = r.url();
    if (r.resourceType() === 'document') return;
    if (/^(data|blob|about|javascript):/.test(url)) return;
    if (url.startsWith(base)) return;
    offOrigin.add(`${r.resourceType()} ${url}`);
  });

  await page.goto(`${base}/${target}`, { waitUntil: 'networkidle' });
  // Reveal-on-scroll gates most of these pages; force it so nothing is measured
  // at opacity 0 mid-transform. A fast programmatic scroll does not reliably
  // fire an IntersectionObserver.
  await page.evaluate(() =>
    document.querySelectorAll('.reveal, .rv, [data-reveal], .fade-in, .anim')
      .forEach(e => e.classList.add('is-visible', 'in', 'is-in', 'visible', 'shown')));
  await page.evaluate(async () => {
    const H = document.body.scrollHeight;
    for (let y = 0; y < H; y += 400) { window.scrollTo(0, y); await new Promise(r => setTimeout(r, 20)); }
    window.scrollTo(0, 0);
  });
  await page.waitForTimeout(400);

  const r = await inspect(page);

  // Identity first: no measurement below is worth anything if this is not the
  // document we think it is.
  const want = TITLE_MUST_MATCH[path];
  if (want && !want.test(r.title)) {
    fail('wrong-page', where,
      `served <title> is “${r.title}”, which does not match ${want} — refusing to trust any measurement`);
    await ctx.close();
    return r;
  }

  if (r.overflow.bleed.length) {
    fail('h-overflow', where,
      `viewport is ${r.overflow.winW}px; elements cross its edge with no clipping ancestor: ` +
      r.overflow.bleed.map(b => `${b.tag}${b.cls ? '.' + b.cls : ''} [${b.left}→${b.right}]`).join('; '));
  }

  if (r.overflow.clipped.length) {
    fail('content-clipped', where,
      'content cut off by an ancestor that does not scroll: ' + r.overflow.clipped
        .map(c => `${c.tag}${c.cls ? '.' + c.cls : ''} loses ${c.lost}px to ${c.by} (overflow-x:${c.overflow})`).join('; '));
  }

  // A scan that matched nothing proves nothing. Say so loudly rather than
  // reporting the resulting silence as a pass.
  if (r.textScanned < 10) {
    fail('vacuous-scan', where,
      `only ${r.textScanned} text-bearing elements found — the legibility floor had nothing to measure`);
  }

  r.smallText.forEach(t => fail('text-too-small', where,
    t.scale === 1
      ? `<${t.tag}> at ${t.effective}px: “${t.text}”`
      : `<${t.tag}> declared ${t.declared}px but drawn at ${t.effective}px (scaled ${t.scale}×): “${t.text}”`));

  r.imgs.forEach(i => {
    if (i.skewPct > 1.5) {
      fail('img-distorted', where,
        `${i.file} drawn ${i.css} from a ${i.nat} source — ${i.skewPct}% off its true aspect ratio`);
    }
    // Vector art has no resolution to fall short of.
    //
    // Note there is deliberately NO srcset precondition here. Basin's version
    // required one, and in a repo that uses no srcset anywhere that made the
    // check apply to exactly zero images while still printing inside a green
    // run — a check whose precondition is never met is a check that does not
    // run.
    const isVector = /\.svgx?(\?|#|$)/i.test(i.file);
    if (!isVector && i.upscale > 1.15) {
      fail('img-soft', where,
        `${i.file} drawn ${i.css} at dpr2 from a ${i.realPx} file — upscaled ${i.upscale}×`);
    }
  });

  r.deadAnchors.forEach(a =>
    fail('dead-anchor', where, `${a.href} (“${a.text}”) matches no id, [name] or chapter route on the page`));

  if (r.chapter) {
    stats.chapterLoads++;
    stats.chapterChars += r.chapter.chars;
    stats.mermaidPainted += r.chapter.mermaidSvgs;
    if (r.chapter.error) fail('chapter-not-loaded', where, 'the chapter body rendered an error placeholder');
    else if (r.chapter.chars < 400 || r.chapter.headings === 0) {
      fail('chapter-not-loaded', where,
        `chapter body holds ${r.chapter.chars} chars and ${r.chapter.headings} headings — the prose did not load, so nothing else here was really measured`);
    }
    if (r.chapter.unpaintedMermaid > 0) {
      fail('mermaid-unpainted', where,
        `${r.chapter.unpaintedMermaid} mermaid block(s) still raw <code>; the renderer did not run`);
    }
  }

  httpErrors.forEach(u => fail('http-error', where, u));
  offOrigin.forEach(u => fail('off-origin', where,
    `${u} — the site must be self-contained; vendor it under site/assets/`));

  await ctx.close();
  return r;
}

// ---------------------------------------------------------------------------
// Cross-page fragment links: index.html ↔ docs.html, routes included.
// ---------------------------------------------------------------------------
async function checkCrossPageAnchors(browser, base) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();

  const idsOf = async (p) => {
    await page.goto(`${base}/${p}`, { waitUntil: 'networkidle' });
    await page.waitForTimeout(400);
    return page.evaluate(() => ({
      ids: [...document.querySelectorAll('[id]')].map(e => e.id),
      routes: [...document.querySelectorAll('[data-slug]')].map(e => e.dataset.slug),
    }));
  };
  const linksOf = async (p) => {
    await page.goto(`${base}/${p}`, { waitUntil: 'networkidle' });
    await page.waitForTimeout(300);
    return page.evaluate(() => [...document.querySelectorAll('a[href*=".html#"]')]
      .map(a => ({ href: a.getAttribute('href'), text: a.textContent.trim().slice(0, 30) })));
  };

  const known = {};
  for (const p of PAGES) {
    if (!existsSync(join(SITE, p))) continue;
    const { ids, routes } = await idsOf(p);
    known[p] = new Set([...ids, ...routes]);
    note(`${p}: ${ids.length} addressable ids + ${routes.length} chapter routes`);
  }
  for (const from of Object.keys(known)) {
    for (const l of await linksOf(from)) {
      const [file, frag] = l.href.replace(/^\.\//, '').split('#');
      if (!known[file]) continue;                       // external or unknown target
      if (!known[file].has(frag) && !known[file].has(frag.split('/')[0])) {
        fail('dead-anchor', `${from} → ${l.href}`,
          `“${l.text}” points at #${frag}, which ${file} defines as neither an id nor a route`);
      }
    }
  }
  await ctx.close();
}

// ---------------------------------------------------------------------------
// Every remaining chapter, at the two widths where the defects live: 320, where
// prose and tables bleed, and 1440, where diagrams are drawn.
// ---------------------------------------------------------------------------
async function checkAllChapters(browser, base) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  await page.goto(`${base}/docs.html`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(400);
  const slugs = await page.evaluate(() =>
    [...new Set([...document.querySelectorAll('[data-slug]')].map(e => e.dataset.slug))]);
  await ctx.close();

  for (const slug of slugs) {
    if (MATRIX_TARGETS.includes(`docs.html#${slug}`)) continue;
    for (const vp of [{ w: 320, h: 720, label: 'phone-min' }, { w: 1440, h: 900, label: 'desktop' }]) {
      await checkPage(browser, base, `docs.html#${slug}`, 'dark', vp);
    }
  }
  note(`swept ${slugs.length} docs chapters at 320 and 1440`);
  return slugs.length;
}

// ---------------------------------------------------------------------------
// Self-test: break each invariant on purpose and require the check to notice.
//
// A gate that has quietly stopped failing looks exactly like one that works —
// this fleet has found roughly twenty-two of them printing PASS while examining
// nothing. So every case plants a real defect and then asserts the measurement
// changes. A case whose mechanism a page does not use says so with the reason
// rather than passing silently; an inapplicable check must never read as a
// working one.
// ---------------------------------------------------------------------------
const CASES = [
  {
    name: 'h-overflow',
    target: 'index.html',
    why: 'a 2400px div parked straight on <body> — the plain case, with the page\'s own overflow rules untouched',
    // Straight onto <body>: the probe treats anything inside an overflow
    // container as contained, and these pages have several, so a div planted
    // inside one could never be seen and the case would report a false MISSED.
    mutate: () => {
      const d = document.createElement('div');
      d.id = '__probe_overflow__';
      d.style.cssText = 'width:2400px;height:20px;background:red';
      document.body.appendChild(d);
      return true;
    },
    caught: r => r.overflow.bleed.length > 0,
    // The other half of the proof: with the mutation gone the same probe must
    // report zero, or "caught" only meant the probe always fires.
    restore: () => { document.getElementById('__probe_overflow__')?.remove(); return true; },
    clean: r => r.overflow.bleed.length === 0,
  },
  {
    name: 'h-overflow (hidden behind overflow-x:clip)',
    target: 'index.html',
    // Measured on this machine, 2400px child in a 1440 viewport:
    //   body{overflow-x:clip}        de.scrollWidth 2408 — caught either way
    //   html{overflow-x:clip}        de.scrollWidth 2408 — caught either way
    //   html,body{overflow-x:clip}   de.scrollWidth 1440 — VACUOUS
    //   html,body{overflow-x:hidden} de.scrollWidth 1440, body.scrollWidth 2400
    // So the arrangement that defeats a scrollWidth assertion is clip on the
    // root AND the body, and that is what this case plants. The property alone
    // tells you nothing — this is why the gate measures geometry instead.
    why: 'the same 2400px child under html,body{overflow-x:clip} — the arrangement that makes documentElement.scrollWidth lie',
    mutate: () => {
      document.documentElement.style.setProperty('overflow-x', 'clip', 'important');
      document.body.style.setProperty('overflow-x', 'clip', 'important');
      const d = document.createElement('div');
      d.id = '__probe_clip__';
      d.style.cssText = 'width:2400px;height:20px;background:red';
      document.body.appendChild(d);
      return true;
    },
    caught: r => r.overflow.bleed.length > 0,
    // Do not reason about the property — measure it. This prints what a
    // scrollWidth assertion would have concluded about the very same page.
    extra: async (page) => {
      const m = await page.evaluate(() => ({
        doc: document.documentElement.scrollWidth,
        client: document.documentElement.clientWidth,
        body: document.body.scrollWidth,
      }));
      return `measured on this page: documentElement.scrollWidth=${m.doc} clientWidth=${m.client} ` +
        `body.scrollWidth=${m.body} — ` +
        (m.doc <= m.client + 1
          ? 'a `scrollWidth - clientWidth` assertion would have passed vacuously here; the geometric probe is what catches it'
          : 'scrollWidth happens to catch this one too, but it is not what this gate relies on');
    },
    restore: () => {
      document.getElementById('__probe_clip__')?.remove();
      document.body.style.removeProperty('overflow-x');
      document.documentElement.style.removeProperty('overflow-x');
      return true;
    },
    clean: r => r.overflow.bleed.length === 0,
  },
  {
    name: 'content-clipped',
    target: 'index.html',
    why: 'a wide image inside an overflow:hidden card — the reader loses the right-hand third and nothing scrolls to reach it',
    // Built rather than borrowed: pointing the mutation at whatever image the
    // page happens to have first made it depend on that element's own layout
    // (a flex child ignores an inline width, an inline host ignores overflow),
    // and the case reported a false MISSED on a check that works.
    mutate: () => {
      const src = [...document.querySelectorAll('img')].find(e => e.naturalWidth > 0);
      const box = document.createElement('div');
      box.id = '__probe_clipbox__';
      box.style.cssText = 'overflow:hidden;width:200px;height:80px;display:block';
      document.body.appendChild(box);
      const im = src ? src.cloneNode(true) : document.createElement('div');
      im.id = '__probe_clipped__';
      im.style.cssText = 'width:900px;max-width:none;height:80px;display:block;background:red';
      if (!src) im.textContent = 'content the reader cannot reach';
      box.appendChild(im);
      return true;
    },
    caught: r => r.overflow.clipped.some(c => c.lost > 600),
    // The distinction the check turns on: with the same overflow set to auto the
    // reader can pan to the rest, and it must go quiet.
    restore: () => {
      document.getElementById('__probe_clipbox__')?.style.setProperty('overflow-x', 'auto', 'important');
      return true;
    },
    clean: r => r.overflow.clipped.length === 0,
  },
  {
    name: 'text-too-small (declared)',
    target: 'index.html',
    why: 'a paragraph dropped to 9px',
    mutate: () => {
      const p = [...document.querySelectorAll('p, li, td')]
        .find(e => e.textContent.trim().length > 40 && e.getBoundingClientRect().width > 40);
      if (!p) return false;
      p.style.setProperty('font-size', '9px', 'important');
      return true;
    },
    caught: r => r.smallText.some(t => t.effective < 10 && t.scale === 1),
  },
  {
    name: 'text-too-small (CSS transform, declared value untouched)',
    target: 'index.html',
    why: 'a paragraph scaled to 0.5 by a transform — the declared font-size never changes, so a property read passes',
    mutate: () => {
      const p = [...document.querySelectorAll('p, li')]
        .find(e => e.textContent.trim().length > 40 && parseFloat(getComputedStyle(e).fontSize) >= 12);
      if (!p) return false;
      p.id = '__probe_scaled__';
      p.style.setProperty('transform', 'scale(0.5)', 'important');
      return true;
    },
    caught: r => r.smallText.some(t => t.scale < 0.95 && t.declared >= 12),
    extra: async (page) => {
      const declared = await page.evaluate(() =>
        parseFloat(getComputedStyle(document.getElementById('__probe_scaled__')).fontSize));
      return declared >= 12
        ? `declared font-size is still ${declared}px — the finding came from the render, not the property`
        : `WARNING: declared font-size fell to ${declared}px, so this case does not prove render-based measurement`;
    },
  },
  {
    name: 'text-too-small (svg viewBox scaling, declared value untouched)',
    target: 'index.html',
    why: 'an svg squeezed so its viewBox maps 16px labels under the floor — exactly the 3.71px mermaid regression, and invisible both to font-size and to getComputedStyle, which reports NOMINAL user units inside an svg',
    mutate: () => {
      let svg = [...document.querySelectorAll('svg[viewBox]')].find(s =>
        [...s.querySelectorAll('text')].some(t => parseFloat(getComputedStyle(t).fontSize) >= 12));
      let planted = false;
      if (!svg) {
        // The page has no SVG of its own carrying text. Plant one rather than
        // report n/a: the mechanism under test is the gate's getScreenCTM path,
        // and it is exactly as load-bearing on a page that grows its first
        // diagram tomorrow.
        const host = document.createElement('div');
        host.innerHTML = '<svg data-probe="1" viewBox="0 0 400 100" width="400" ' +
          'style="display:block"><text x="10" y="50" font-size="16" fill="currentColor">' +
          'a label that is legible at full size</text></svg>';
        document.body.appendChild(host);
        svg = host.firstChild;
        planted = true;
      }
      svg.dataset.probe = '1';
      svg.dataset.planted = planted ? '1' : '0';
      svg.style.setProperty('width', '90px', 'important');
      svg.style.setProperty('max-width', '90px', 'important');
      svg.style.setProperty('height', 'auto', 'important');
      return true;
    },
    caught: r => r.smallText.some(t => t.scale < 0.95 && t.declared >= 12 && (t.tag === 'text' || t.tag === 'tspan')),
    extra: async (page) => {
      const m = await page.evaluate(() => {
        const svg = document.querySelector('svg[data-probe="1"]');
        const t = svg.querySelector('text');
        const ctm = svg.getScreenCTM();
        return {
          planted: svg.dataset.planted === '1',
          declared: parseFloat(getComputedStyle(t).fontSize),
          scale: +Math.sqrt(Math.abs(ctm.a * ctm.d - ctm.b * ctm.c)).toFixed(3),
        };
      });
      return `${m.planted ? 'no svg on this page carries text, so one was planted; ' : 'used an svg already on the page; '}` +
        `getComputedStyle still reports ${m.declared}px while getScreenCTM reports a ${m.scale}× map ` +
        `— the finding came from the render, not the property`;
    },
  },
  {
    name: 'text-too-small (::before content)',
    target: 'docs.html',
    why: 'a generated-content label dropped to 9px — text the element scan cannot see, because a pseudo-element owns no node',
    mutate: () => {
      const el = [...document.querySelectorAll('body *')].find(e => {
        const c = getComputedStyle(e, '::before').content;
        return /^".{2,}"$/s.test(c) && getComputedStyle(e).display !== 'none'
          && e.getBoundingClientRect().width > 2;
      });
      if (!el) {
        // No page-authored ::before text to shrink. Plant one: the scan's
        // ability to see generated content is the thing under test.
        const s = document.createElement('style');
        s.textContent = '#__probe_pseudo__::before{content:"generated label";font-size:9px}';
        document.head.appendChild(s);
        const d = document.createElement('div');
        d.id = '__probe_pseudo__';
        document.body.appendChild(d);
        return true;
      }
      const s = document.createElement('style');
      s.textContent = '.__probe_pseudo__::before{font-size:9px !important}';
      document.head.appendChild(s);
      el.classList.add('__probe_pseudo__');
      return true;
    },
    caught: r => r.smallText.some(t => t.tag.includes('::before') && t.effective < 10),
  },
  {
    name: 'img-distorted',
    target: 'index.html',
    why: 'a decoded image forced to object-fit:fill at double height',
    mutate: () => {
      const box = e => e.getBoundingClientRect();
      // Must be a DECODED image — the check skips anything with no intrinsic
      // size — and object-fit must be forced to `fill`, because skew under
      // cover/contain is ignored BY DESIGN and the case would report a false
      // MISSED on a cropped image.
      const im = [...document.querySelectorAll('img')]
        .filter(e => box(e).width > 8 && box(e).height > 8 && e.naturalWidth > 0)
        .sort((a, b) => box(b).width - box(a).width)[0];
      if (!im) return false;
      im.style.setProperty('object-fit', 'fill', 'important');
      im.style.setProperty('height', Math.round(box(im).width * 2) + 'px', 'important');
      im.style.setProperty('aspect-ratio', 'auto', 'important');
      return true;
    },
    caught: r => r.imgs.some(i => i.skewPct > 1.5),
  },
  {
    name: 'img-soft',
    target: 'index.html',
    why: 'a raster image drawn far above the resolution of its source file',
    mutate: () => {
      const im = [...document.querySelectorAll('img')].find(e =>
        e.naturalWidth > 0 && !/\.svg(\?|#|$)/i.test(e.currentSrc || e.src) &&
        e.getBoundingClientRect().width > 8);
      if (!im) return false;
      im.style.setProperty('width', '2000px', 'important');
      im.style.setProperty('max-width', 'none', 'important');
      im.style.setProperty('height', 'auto', 'important');
      return true;
    },
    caught: r => r.imgs.some(i => i.upscale > 1.15),
  },
  {
    name: 'dead-anchor',
    target: 'index.html',
    why: 'a working fragment link repointed at an id nothing defines',
    // A landing whose nav links are all absolute has no resolvable fragment
    // link to break. Reporting n/a there would leave the check untested on the
    // page it runs against every commit, so plant the working pair first — the
    // restore leg then proves the scan reports zero on the working version.
    mutate: () => {
      let a = [...document.querySelectorAll('a[href^="#"]')]
        .find(e => e.getAttribute('href').length > 1 && document.getElementById(e.getAttribute('href').slice(1)));
      if (!a) {
        const target = document.createElement('div');
        target.id = '__probe_target__';
        document.body.appendChild(target);
        a = document.createElement('a');
        a.id = '__probe_anchor__';
        a.href = '#__probe_target__';
        a.textContent = 'planted link';
        document.body.appendChild(a);
      }
      a.dataset.probeWas = a.getAttribute('href');
      a.setAttribute('href', '#section-that-does-not-exist');
      return true;
    },
    caught: r => r.deadAnchors.some(a => a.href === '#section-that-does-not-exist'),
    restore: () => {
      const a = document.querySelector('a[data-probe-was]');
      if (a) a.setAttribute('href', a.dataset.probeWas);
      return true;
    },
    clean: r => !r.deadAnchors.some(a => a.href === '#section-that-does-not-exist'),
  },
  {
    name: 'off-origin',
    target: 'index.html',
    why: 'an <img> pointed at a host that is certainly not this test server — the request leaving is the defect, it never has to load',
    mutate: () => {
      const im = document.createElement('img');
      im.src = 'https://cdn.example.invalid/pixel.png';
      im.alt = '';
      document.body.appendChild(im);
      return true;
    },
    caught: (r, page, ctxState) => ctxState.offOrigin.size > 0,
  },
  {
    name: 'chapter-not-loaded',
    target: 'docs.html',
    why: 'the fetched prose emptied out, as a 404 on site/docs/*.md would leave it',
    mutate: () => {
      const c = document.querySelector('#content');
      if (!c) return false;
      c.innerHTML = '';
      return true;
    },
    caught: r => !!r.chapter && (r.chapter.chars < 400 || r.chapter.headings === 0),
  },
  {
    name: 'mermaid-unpainted',
    target: 'docs.html',
    why: 'a mermaid block left as raw <code>, the shape the page takes when the vendored renderer fails to load',
    mutate: () => {
      const c = document.querySelector('#content');
      if (!c) return false;
      const pre = document.createElement('pre');
      pre.innerHTML = '<code class="language-mermaid">graph TD; A--&gt;B;</code>';
      c.appendChild(pre);
      return true;
    },
    caught: r => !!r.chapter && r.chapter.unpaintedMermaid > 0,
  },
  {
    name: 'vacuous-scan',
    target: 'index.html',
    why: 'the page stripped of its text — the shape a scan takes when its selector stops matching',
    mutate: () => { document.body.innerHTML = '<p>x</p>'; return true; },
    caught: r => r.textScanned < 10,
  },
];

// The identity assertion cannot be mutation-tested from inside the page: the
// point is that the SERVER might be somebody else's. So stand up a decoy that
// answers with a different product's document and require the gate to refuse
// it. This is the exact failure that once produced 410 green widths against the
// wrong site with nothing in the output looking wrong.
async function selftestIdentity(browser) {
  const decoy = await new Promise(ok => {
    const s = createServer((_req, res) => {
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
      res.end('<!doctype html><title>Basin — object storage for your own hardware</title>' +
        '<body><main><p>' + 'a decoy page that is not this product. '.repeat(40) + '</p></main>');
    });
    s.listen(0, '127.0.0.1', () => ok(s));
  });
  const base = `http://127.0.0.1:${decoy.address().port}`;
  const before = findings.length;
  await checkPage(browser, base, 'index.html', 'dark', { w: 1440, h: 900, label: 'decoy' });
  const planted = findings.splice(before);      // do not leak into the real run
  decoy.close();
  const caught = planted.some(f => f.check === 'wrong-page');
  console.log(`  ${caught ? 'caught  ' : 'MISSED  '} wrong-page`);
  console.log(`             a decoy server answered index.html with another product's <title>` +
    (caught ? '; the gate refused to trust the measurement' : '; the gate measured it anyway'));
  if (caught) console.log('             (the real site\'s title passes the same assertion — the run below proves it)');
  return caught;
}

async function selftest(browser, base) {
  let allCaught = true;
  for (const c of CASES) {
    if (!existsSync(join(SITE, c.target.split('#')[0]))) {
      console.log(`  n/a      ${c.name}`);
      console.log(`             this repo has no site/${c.target.split('#')[0]}`);
      continue;
    }
    const vp = c.vp || { w: 1440, h: 900 };
    const ctx = await browser.newContext({
      viewport: { width: vp.w, height: vp.h }, deviceScaleFactor: 2, colorScheme: 'dark',
    });
    const page = await ctx.newPage();
    const ctxState = { offOrigin: new Set() };
    page.on('request', r => {
      const url = r.url();
      if (r.resourceType() === 'document') return;
      if (/^(data|blob|about|javascript):/.test(url)) return;
      if (url.startsWith(base)) return;
      ctxState.offOrigin.add(url);
    });

    await page.goto(`${base}/${c.target}`, { waitUntil: 'networkidle' });
    await page.evaluate(() =>
      document.querySelectorAll('.reveal, .rv, [data-reveal], .fade-in, .anim')
        .forEach(e => e.classList.add('is-visible', 'in', 'is-in', 'visible', 'shown')));
    await page.waitForTimeout(600);

    const applicable = await page.evaluate(`(${c.mutate.toString()})()`);
    if (!applicable) {
      console.log(`  n/a      ${c.name}`);
      console.log(`             ${c.target} has no element this defect could be planted on`);
      allCaught = false;
      await ctx.close();
      continue;
    }
    await page.waitForTimeout(350);
    const r = await inspect(page);
    const caught = !!c.caught(r, page, ctxState);
    console.log(`  ${caught ? 'caught  ' : 'MISSED  '} ${c.name}`);
    console.log(`             ${c.why}`);
    if (c.extra) console.log(`             ${await c.extra(page)}`);
    if (c.restore) {
      await page.evaluate(`(${c.restore.toString()})()`);
      await page.waitForTimeout(200);
      const back = await inspect(page);
      const clean = !!c.clean(back);
      console.log('             with the mutation undone the same probe ' +
        (clean ? 'goes quiet — it is not simply always firing' : 'STILL FIRES, so the probe is unconditional'));
      if (!clean) allCaught = false;
    }
    if (!caught) allCaught = false;
    await ctx.close();
  }
  const identity = await selftestIdentity(browser);
  return allCaught && identity;
}

// ---------------------------------------------------------------------------
async function main() {
  if (!existsSync(join(SITE, 'index.html'))) {
    console.error(`check-render: no site/index.html under ${SITE}`);
    process.exit(2);
  }
  const { chromium } = await loadPlaywright();
  const server = await serve(SITE);
  const base = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch({ headless: true });
  console.log(`check-render: serving ${SITE} on ${base}`);

  try {
    if (process.argv.includes('--selftest')) {
      console.log('check-render self-test — each invariant is broken on purpose:\n');
      const ok = await selftest(browser, base);
      console.log(ok ? '\nSELF-TEST PASS — every applicable check discriminates.'
        : '\nSELF-TEST FAIL — a check did not notice its own defect.');
      process.exitCode = ok ? 0 : 1;
      return;
    }

    let combos = 0, imgs = 0, texts = 0, anchors = 0;
    for (const vp of VIEWPORTS) {
      for (const theme of ['light', 'dark']) {
        for (const target of MATRIX_TARGETS) {
          if (!existsSync(join(SITE, target.split('#')[0]))) continue;
          const r = await checkPage(browser, base, target, theme, vp);
          combos++; imgs += r.imgs.length; texts += r.textScanned; anchors += r.anchorsScanned;
        }
      }
    }
    const chapters = await checkAllChapters(browser, base);
    await checkCrossPageAnchors(browser, base);

    console.log(`\nchecked ${combos} route×width×scheme combinations plus ${chapters} chapters at 2 widths\n` +
      `  ${texts} text runs measured for effective size, ${imgs} rendered images, ${anchors} fragment links\n`);
    notes.forEach(n => console.log('  · ' + n));
    console.log(`  · ${stats.chapterLoads} chapter loads carrying ${stats.chapterChars} chars of prose in total`);
    console.log(stats.mermaidPainted
      ? `  · ${stats.mermaidPainted} mermaid diagram(s) painted to svg; zero left as raw <code>`
      : '  · no mermaid diagrams on these pages — the unpainted-block check guards a mechanism the docs do not currently use, and the self-test plants one to prove it still discriminates');

    if (findings.length) {
      console.error(`\ncheck-render: ${findings.length} finding(s)\n`);
      const byCheck = {};
      findings.forEach(f => (byCheck[f.check] ||= []).push(f));
      for (const [check, list] of Object.entries(byCheck)) {
        console.error(`  ${check} (${list.length})`);
        // Collapse the width dimension: the same defect at eight widths is one
        // defect, and printing it eight times buries the others.
        const seen = new Set();
        list.forEach(f => {
          if (seen.has(f.detail)) return;
          seen.add(f.detail);
          console.error(`    ${f.where}\n      ${f.detail}`);
        });
      }
      process.exitCode = 1;
    } else {
      console.log('\ncheck-render: clean');
    }
  } finally {
    await browser.close();
    server.close();
  }
}

main().catch(e => { console.error(e); process.exit(2); });
