# Contributing to Soko

Soko is the reference implementation of
[TRACT](https://github.com/vul-os/kotva/tree/main/profiles/tract). All contributions are under the
[MIT licence](LICENSE).

## Before anything structural: spec-first discipline

TRACT is being written before the implementation on purpose — the failure mode being avoided is a
protocol that is a description of whatever the first implementation happened to do. That has one
practical consequence for this repository:

**Where Soko and the specification disagree, the specification wins.** If you find TRACT
unimplementable — a check that can't be expressed, a type that doesn't fit a real case — that is a
defect in the spec, not licence to quietly diverge here. Raise it as an issue on
[vul-os/kotva](https://github.com/vul-os/kotva/issues) against the
[TRACT profile](https://github.com/vul-os/kotva/tree/main/profiles/tract), and reference it from the
PR. Do not paper over an unimplementable section by implementing something adjacent to it.

Read [docs/architecture.md](docs/architecture.md) before changing anything structural. The
dependency direction (`everything → soko-core → soko-seam`, fixed from the first commit) and the
`public`/`sealed` split are load-bearing, not stylistic.

## Building and testing

```sh
cargo test --workspace                              # unit tests
cargo clippy --workspace --all-targets -- -D warnings  # lint, warnings are errors
cargo fmt --all -- --check                           # formatting
node tools/diagrams.mjs                              # regenerate docs/diagrams/*.png from source
node tools/sync-docs.mjs                             # mirror docs/ -> site/docs/ after editing docs
```

The three `cargo` commands run in CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)), along
with the review gates described below. A PR that doesn't pass them locally won't pass there either.

The two `node` tools are *generators*, so CI does not run them — it checks their output instead, and
only where a check exists. `cargo test` fails if `site/docs/` has drifted from `docs/` (see below).
Nothing yet checks that `docs/diagrams/*.png` still matches the source that generates them, so
regenerating after editing a diagram is currently discipline rather than a gate.

Regenerate diagrams with the tool rather than hand-editing an image — a diagram maintained as a
static file drifts from the prose it illustrates, and nobody notices until it's wrong.

**If you edit anything under `docs/`, run `node tools/sync-docs.mjs`.** `site/docs.html` is a
client-side renderer that fetches `./docs/<page>.md` at runtime, which resolves to `site/docs/` — so
that tree is the copy the published site actually serves, not a build artefact. The two trees are
byte-identical by contract, and `cargo test` enforces it
(`crates/soko-node/tests/docs_mirror.rs`); editing only `docs/` used to publish stale text with no
signal anywhere. `node tools/sync-docs.mjs --check` reports drift without writing.

## Coverage

```sh
cargo install cargo-llvm-cov --locked   # once
cargo llvm-cov --workspace              # text summary
cargo llvm-cov --workspace --html       # target/llvm-cov/html/index.html, browsable
```

CI runs this on every push and PR (the `coverage` job) and posts the summary to the job's summary
page. **There is no coverage threshold, and none is planned.** A percentage gate rewards tests
written to move the number rather than tests written to catch a bug, and it is gameable by
asserting nothing while merely executing a line. Coverage here is reported so a human can look at
it and ask "why is this at 0%?" about a specific file — which is exactly the question nobody was
asking about `crates/soko-gateway/src/bin/storefront.rs` before it had any tests at all, despite
being the only user-facing surface in the project. If you're reading this because you're about to
add a `--fail-under-lines` or similar: don't — raise it as a discussion first, because the point
above is the reasoning, not an oversight.

## Two hard review gates

These are enforced beyond "please don't":

### `soko-seam` stays at zero dependencies

```sh
cargo tree -p soko-seam    # must be exactly one line
```

CI fails the build if this changes (`seam-has-no-dependencies` job). The seam names no settlement
or storefront provider so that implementing it costs nothing; a dependency added there — even a
small, reasonable one — is inherited by every party who wires their own payment provider, which
defeats the reason the seam exists. If your change needs `soko-seam` to depend on something, the
type almost certainly belongs in `soko-settle` or `soko-gateway` instead, converting at the
boundary. `RailClass` already does this: an in-process variant with no derives in `soko-seam`, a
wire-format variant with `serde` in `soko-settle`, converting between them.

### Nothing personal enters the `public` module

`soko-core` splits `public` (signed, content-addressed, globally deduplicated, **irrevocable**)
from `sealed` (encrypted to the counterparties, deletable at the edges). A right to erasure cannot
be satisfied against an irrevocable object, so the rule is structural rather than a comment: a
`public` type must be incapable of carrying a name, address, or contact detail, full stop. If a
review finds personal data reachable from a `public` type — even behind an `Option`, even in a test
fixture that looks disposable — that blocks the PR regardless of what else it does. See
[docs/architecture.md](docs/architecture.md#the-publicsealed-split-is-structural) for the reasoning
and the one bounded exception (reviews, via pseudonymous subkeys and superseding tombstones).

## What this repository does not implement

Two things are explicitly out of scope for a PR here, however good the reason seems:

- **Cryptography.** Identity, signing, content addressing, feeds and sync are the
  [DMTAP substrate](https://github.com/vul-os/kotva)'s job. A hash construction or signature
  framing invented in `soko-core` is a bug, not a contribution.
- **A settlement rail.** `soko-seam` is a provider-agnostic contract; [patala](https://github.com/vul-os/patala)
  is one implementation of it. A PR that wires a specific payment provider into `soko-seam` or
  `soko-settle` directly, rather than behind the seam, will be redirected.

## Commit style

Commits carry an explanatory body that says **why**, not just what — read `git log` before writing
one to see the shape expected. A one-line `fix: thing` with no body is fine for something genuinely
trivial; anything that changes behaviour, a type's shape, or a review gate should explain the
reasoning that led there, including what was tried and rejected if that's relevant.

Commits are authored as `imranparuk`, with no co-author trailer. If your contribution is via PR,
this is handled at merge time — you don't need to match it in your own commits.

## Workflow

1. Fork, branch from `main`.
2. Make the change. If it touches a crate whose TRACT section isn't fully specified yet, say so in
   the PR — the crate's own doc comment should already note how far its section has been specified,
   and a change that outruns the spec needs the spec caught up first, not shipped ahead of it.
3. Run the four commands above.
4. Open a PR describing what changed and why. Link the TRACT section (`§N`) it implements.

## Status

Pre-alpha. There is no packaged tool and no UI — contributions land in the crates and the docs.
See [docs/roadmap.md](docs/roadmap.md) for where the specification stands and
[ROADMAP.md](ROADMAP.md) for repository sequencing.

## Where help is most needed

Beyond ordinary contributions, several items are blocked on expertise rather than effort — data
protection law, EU VAT, carrier API terms, privacy-preserving measurement, and reputation attack
literature. Each had research passes that returned nothing verifiable, and each is recorded with
its specific unanswered question in
[TRACT's help-wanted](https://github.com/vul-os/kotva/blob/main/profiles/tract/docs/HELP-WANTED.md).

The single most valuable contribution is an implementation of TRACT that does **not** read this
code. One implementation and one specification derived from each other only prove they agree with
one another; a second, independent one is what would establish that the document is buildable from.
