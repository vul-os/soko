# Vendored TRACT conformance vectors

`vectors/` holds a byte-for-byte copy of the TRACT specification's conformance corpus — 21 JSON
files, derived by hand from the specification text, never exported from this workspace.
`PROVENANCE.json` records a BLAKE3 digest of every one of them and the upstream commit they were
copied from.

Upstream: <https://github.com/vul-os/kotva>, `profiles/tract/conformance/vectors`. The TRACT
specification is licensed **CC BY 4.0** (see `profiles/tract/LICENSE.md` there), which is what makes
copying it here legitimate; this directory is the attribution.

## Why a copy, when the corpus exists upstream

`tests/conformance.rs` used to read the vectors out of a sibling `../tract` checkout and skip when
one was absent. TRACT has since moved into the KOTVA repository as a profile, so `../tract` stopped
resolving anywhere — and the skip was invisible, because `cargo test` captures a passing test's
`println!` output and throws it away. The harness printed `ok` and verified nothing, for long
enough that three of its own "no Soko function computes this" notes went stale while nobody could
see it.

A vendored copy fixes the failure mode at its root rather than making the notice louder: there is
no longer a condition under which the conformance checks do not run. A bare `git clone` of this
repository, with no network and no sibling checkout, runs every one of them.

## What checks what

| Check (in `tests/conformance.rs`) | Fails when |
|---|---|
| `vendored_corpus_matches_its_provenance` | a vendored file was edited, added or removed without re-recording `PROVENANCE.json` |
| `vendored_corpus_census_matches_the_files_on_disk` | the corpus grew, shrank, or a vector gained a case the harness's `CORPUS` table does not know about |
| `tract_hand_derived_vectors_agree_with_soko` | Soko disagrees with a hand-derived vector, **or** the run performed fewer checks than a full run performs |
| `vendored_corpus_matches_the_upstream_spec_checkout` | the vendored copy has drifted from a spec checkout that is present |

The first three need nothing but this repository. The fourth is the only one that can be skipped,
and it is the only one that *should* be: a soko-only checkout can verify Soko against the corpus,
but nothing can verify the corpus against a specification that is not there.

Its skip is written to the process's real `stderr` handle rather than through `println!`/`eprintln!`,
because libtest's output capture applies to the print macros and not to the handle — measured:

```
$ cargo test          # no --nocapture
...
============================================================================
SKIPPED: vendored_corpus_matches_the_upstream_spec_checkout

NOT VERIFIED in this run: that the 21 vendored TRACT conformance vectors in
crates/soko-node/tests/tract-vectors/vectors/ still match the spec repository.
...
```

CI that *does* check the spec out sets `SOKO_REQUIRE_TRACT_VECTORS=1`, which turns that skip into a
failure — a missing checkout there is a misconfigured job, not an unusual machine.

## Re-vendoring

When the corpus changes upstream:

```sh
SOKO_TRACT_VECTORS=../kotva/profiles/tract/conformance/vectors \
  cargo test -p soko-node --test conformance -- --ignored refresh_vendored_tract_vectors
```

That rewrites `vectors/` and `PROVENANCE.json` together. It is a maintenance action, and
deliberately **not** a way to fix a failing conformance check: if Soko and a vector disagree, the
specification text is the tiebreaker and one of the two is wrong. Re-vendoring to make a
disagreement go away is the exact failure `conformance/README.md` upstream exists to prevent.
