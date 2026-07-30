#!/usr/bin/env node
// Mirror docs/ into site/docs/.
//
// WHY THIS EXISTS
//
// site/docs.html is a client-side renderer: it fetches `./docs/<page>.md` at
// runtime (see the `pages` table in that file) and renders the markdown in the
// browser. Relative to site/, that path is site/docs/ — so site/docs/ is not a
// build artefact nobody reads, it is the copy the published site actually
// serves. docs/ is the copy contributors edit.
//
// Those two trees were 25 byte-identical files with nothing keeping them that
// way. Editing docs/ and forgetting site/docs/ published stale text with no
// signal at all: no build step touched it, no test compared them, and the
// duplication was invisible because the files agreed. This script is the
// missing copy step.
//
// THE GATE IS NOT THIS SCRIPT
//
// `--check` exits non-zero on drift and is useful locally, but the gate that
// actually runs on every push is a Rust test —
// crates/soko-node/tests/docs_mirror.rs — because that runs under the
// `cargo test --workspace` this repository already gates on, needs no Node
// toolchain in CI, and cannot be skipped into silence. A CI step that needs a
// runtime the job does not install fails for the wrong reason, or worse, gets
// wrapped in an `if` that quietly stops checking.
//
// USAGE
//   node tools/sync-docs.mjs          # copy docs/ -> site/docs/
//   node tools/sync-docs.mjs --check  # report drift, exit 1, write nothing

import { readdirSync, readFileSync, writeFileSync, mkdirSync, rmSync, statSync, existsSync } from 'node:fs'
import { join, relative, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const SRC = join(ROOT, 'docs')
const DST = join(ROOT, 'site', 'docs')
const CHECK = process.argv.includes('--check')

/** Every file under `dir`, as repo-relative-to-`dir` paths, sorted. */
function walk(dir, base = dir) {
  const out = []
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const abs = join(dir, entry.name)
    if (entry.isDirectory()) out.push(...walk(abs, base))
    else if (entry.isFile()) out.push(relative(base, abs))
  }
  return out.sort()
}

function die(msg) {
  console.error(`sync-docs: ${msg}`)
  process.exit(1)
}

// Fail closed on a source tree that is missing or empty. A mirror check that
// passes because there was nothing to mirror is worse than no check: it reports
// success for a checkout that is actually broken.
if (!existsSync(SRC) || !statSync(SRC).isDirectory()) {
  die(`${relative(ROOT, SRC)}/ does not exist. This is the source of the mirror; the checkout is broken.`)
}
const sources = walk(SRC)
if (sources.length === 0) {
  die(`${relative(ROOT, SRC)}/ holds no files. Refusing to report a mirror as in sync when there is nothing in it.`)
}

const existing = existsSync(DST) ? walk(DST) : []
const stale = existing.filter((f) => !sources.includes(f))

const toCopy = sources.filter((f) => {
  const dst = join(DST, f)
  if (!existsSync(dst)) return true
  return !readFileSync(join(SRC, f)).equals(readFileSync(dst))
})

if (CHECK) {
  for (const f of toCopy) console.error(`  drift  site/docs/${f}`)
  for (const f of stale) console.error(`  stale  site/docs/${f}`)
  if (toCopy.length || stale.length) {
    console.error(
      `\nsync-docs: site/docs/ is out of date (${toCopy.length} to copy, ${stale.length} stale).\n` +
        `site/docs.html fetches these files at runtime, so this drift is published text.\n` +
        `Run \`node tools/sync-docs.mjs\` and commit the result.`,
    )
    process.exit(1)
  }
  console.log(`sync-docs: site/docs/ matches docs/ (${sources.length} files).`)
  process.exit(0)
}

for (const f of stale) {
  rmSync(join(DST, f))
  console.log(`  remove  site/docs/${f}`)
}
for (const f of toCopy) {
  const dst = join(DST, f)
  mkdirSync(dirname(dst), { recursive: true })
  writeFileSync(dst, readFileSync(join(SRC, f)))
  console.log(`  sync    site/docs/${f}`)
}
console.log(
  `sync-docs: ${sources.length} files mirrored, ${toCopy.length} written, ${stale.length} removed.`,
)
