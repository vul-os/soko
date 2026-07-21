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
  the first commit: everything depends on `soko-core`, `soko-core` depends on `soko-seam`,
  `soko-seam` depends on nothing.
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

### Known limits at this stage

- No wire-format interoperability has been demonstrated — there is no second implementation to
  interoperate with yet, and conformance vectors don't exist.
- Several crates (`soko-core`, `soko-seam`, `soko-node`) have no unit tests of their own; their
  types are exercised indirectly through the crates built on top of them.
- No end-to-end flow — publish a catalogue, place an order, settle a payment — has been run. The
  crates compile and their internal logic is tested; nothing has been wired together as a program.

[Unreleased]: https://github.com/vul-os/soko/commits/main
