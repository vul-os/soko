# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project has not made a release yet;
versions on `main` are tracked under [Unreleased] until the first tag.

## [Unreleased]

Pre-alpha. There is no runnable product, no packaged binary, and no UI. What exists is the
protocol's type surface, a workspace that compiles, and a docs/site pair kept honest about what is
and isn't there yet.

### Added

- Workspace scaffolding for all 11 crates named in TRACT, with the dependency direction fixed from
  the first commit: everything depends on `soko-core`, and `soko-seam` depends on nothing. (The
  original wording had `soko-core` depending on `soko-seam`; it declared that dependency without
  ever naming it, and the declaration has since been removed — `soko-settle` is the one crate that
  uses the seam.)
  - `soko-seam` (§9, §12) — settlement / escrow / storefront traits naming no provider. Zero
    dependencies, enforced in CI.
  - `soko-core` (§16) — content addresses, money, places, time, and the `public` / `sealed` type
    split.
  - `soko-offer` (§3–§5) — the four axes (item, availability, fulfilment, consideration), and
    `place_of_supply_kind` deriving the tax anchor from fulfilment.
  - `soko-catalog` (§2) — product records, product groups, the identity ladder, canonicalisation.
  - `soko-order` (§6–§7) — buyer-held cart, per-seller split, `BoundedCounter` inventory, sealed
    order state machine.
  - `soko-delivery` (§8) — rate cards, volumetric weight, legs, consignments, distributors, route
    comparison.
  - `soko-settle` (§9) — payment attestations, rail classes, `EscrowScope::check` with fail-closed
    intersection.
  - `soko-trust` (§10) — purchase-attested reviews, per-index `Weighting`, `local_score`.
  - `soko-jurisdiction` (§11) — the four anchors, `place_of_supply` resolution,
    `ResponsibleParties::may_offer_into`.
  - `soko-gateway` (§12) — storefront store bindings and `origin_isolated_from`.
  - `soko-node` — the node binary. Does not host the gateway role; the gateway runs as a separate
    process with no access to identity keys or the object store.
- 37 tests, concentrated where getting it wrong is silent rather than loud: volumetric weight,
  currency mismatch on a route total, escrow scope intersection (including place-of-supply-only
  mismatches), place of supply for an event held abroad, concurrent replicas not overselling
  bounded-counter inventory, and unattested reviews not moving a conservative score.
- CI: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and a dedicated job asserting `cargo tree -p soko-seam` stays one line.
- Documentation set (`docs/`): overview, architecture, threat model, catalogue, offers, delivery,
  settlement, trust, jurisdiction, gateway, self-hosting, analytics, protocol reference, FAQ, and
  the repository's `docs/roadmap.md` tracking the TRACT sections themselves.
- Landing page and docs site under `site/`, and `tools/diagrams.mjs` rendering the architecture
  diagrams from a single mermaid source rather than hand-maintained images.
- MIT licence for this implementation; TRACT itself is licensed separately under CC BY 4.0.

### Fixed

- The conformance harness verified nothing. It read the TRACT vectors from a sibling `../tract`
  checkout, and when that was absent it printed a skip and returned — but `cargo test` discards a
  *passing* test's output, so the skip was invisible, and TRACT had meanwhile moved into the KOTVA
  repository as a profile, so `../tract` resolved nowhere. The corpus is now vendored into
  `crates/soko-node/tests/tract-vectors/` with a BLAKE3 digest per file, so the checks run in any
  checkout with no network and no sibling repository; a separate check compares the vendored copy
  against a spec checkout where one is present, and writes its skip to the process's real stderr,
  which libtest does not capture.
- Three of that harness's "no Soko function computes this" notes were stale. `Zone::select_bracket`
  and `EscrowScope::intersect` had been written specifically because the vectors asked for them,
  and nothing noticed they were now checkable, because the file that would have said so never ran.
  Both are checked now: 21 vector cases agree, up from 13 reported and 0 actually executed.
- Removed five dependency declarations no source file referenced: `soko-seam` from `soko-core`,
  `soko-gateway` and `soko-node`; `thiserror` from `soko-erasure`; `soko-offer` from `soko-order`.
- **Every link to the TRACT specification 404'd.** When TRACT moved into the KOTVA repository as a
  profile, the harness and CI were repointed but the 18 human-facing references to
  `github.com/vul-os/tract` were not — in the README (including the protocol badge), CONTRIBUTING,
  SECURITY, `docs/overview.md`, `docs/protocol.md`, the `site/` landing page (including a
  `git clone` line that could not work) and their `site/docs/` mirrors. `crates/soko-node/src/main.rs`
  printed it **to the user on every run** of `soko-node` with no argument. Unlike the `dmtap` →
  `kotva` rename, which GitHub still redirects, `tract` was folded in rather than renamed, so there
  was no redirect to soften it. All now point at
  `github.com/vul-os/kotva/tree/main/profiles/tract` (and the specific files under it, for the §21
  grounding, help-wanted and security-policy links), which is the location the vendored corpus's
  `PROVENANCE.json` and the `tract-vector-drift` CI job already recorded. The `dmtap` URLs were
  repointed to `kotva` at the same time: they resolve today only via a rename redirect, which GitHub
  drops if anything ever re-takes the old name.
- **`docs/` and `site/docs/` were 25 duplicated files with nothing keeping them in step.**
  `site/docs.html` is a client-side renderer that fetches `./docs/<page>.md` at runtime, so
  `site/docs/` is the copy the published site serves rather than a build artefact — and editing
  `docs/` alone published stale text with no signal anywhere, the duplication being invisible
  precisely because the two copies agreed. `tools/sync-docs.mjs` now performs the copy
  (`--check` reports drift without writing), and `crates/soko-node/tests/docs_mirror.rs` is the
  gate. It is a Rust test so that it runs under the `cargo test --workspace` CI already gates on,
  needing no Node toolchain in the job and leaving no `if` for it to be wrapped in. It fails closed:
  a missing tree, an *empty* source tree, or an unreadable file is a failure, never a skip.

### Known limits at this stage

- No wire-format interoperability has been demonstrated — there is no second implementation to
  interoperate with yet, and conformance vectors don't exist.
- Several crates (`soko-core`, `soko-seam`, `soko-node`) have no unit tests of their own; their
  types are exercised indirectly through the crates built on top of them.
- No end-to-end flow — publish a catalogue, place an order, settle a payment — has been run. The
  crates compile and their internal logic is tested; nothing has been wired together as a program.

[Unreleased]: https://github.com/vul-os/soko/commits/main
