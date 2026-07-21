# Roadmap

> Written as sequence, not dates. This page tracks the **repository** — crates, tests, releases.
> For the specification itself, see [docs/roadmap.md](docs/roadmap.md); that page tracks which
> TRACT sections are normative, this one tracks how far Soko's crates go in implementing them.

## Where things actually stand

Every crate in the workspace compiles and carries real domain types — this is not a set of empty
modules claiming coverage. But "compiles with types" and "does something" are different claims, and
conflating them is the exact kind of dishonesty this project exists to avoid elsewhere. Concretely,
right now:

- The type surface for all 11 crates exists, matched against the TRACT sections it covers (see the
  table in [README.md](README.md) or [docs/architecture.md](docs/architecture.md)).
- 37 tests pin behaviour at the points where getting it wrong is silent: volumetric weight, currency
  mismatch, escrow scope intersection, place of supply, oversell across replicas, reputation
  scoring.
- Nothing is wired together as a program. There is no catalogue you can publish, no order you can
  place, no node you can run against another node. `soko-node` exists as a crate; it does not yet
  do anything a seller could point at.

## Sequence

### Stage 0 — types and unit behaviour (in progress)

Each crate's types get deep enough to be worth testing in isolation, matching how far its TRACT
section has been written. A crate whose section is still a stub in the spec stays shallow here on
purpose — implementing ahead of the spec is exactly the drift this project is structured to avoid.

- [x] Dependency direction fixed and enforced in CI (`soko-seam` zero-dependency gate)
- [x] `public` / `sealed` split structural in `soko-core`
- [x] Bounded-counter inventory with a partition test proving no oversell
- [x] Rail-class / escrow-scope types with fail-closed intersection
- [ ] Deepen `soko-core` and `soko-seam` themselves — currently the two crates with no unit tests of
      their own, exercised only indirectly through crates built on top

### Stage 1 — a catalogue you can publish and verify

The first thing that has to work end to end, because everything downstream depends on it existing.

- [ ] `soko-catalog`: sign a product record, publish it as a feed, have a second node fetch and
      verify it — over the DMTAP substrate once its crates are available (see
      [Cargo.toml](Cargo.toml), currently commented placeholders)
- [ ] Canonicalisation proven stable across a real serialize/deserialize round trip, not just typed

### Stage 2 — a cart and an order between two nodes

- [ ] `soko-order`: buyer-side cart as CRDT state, tested for convergence under concurrent edits
- [ ] A sealed order sent from a buyer's node to a seller's node, with signed state transitions
- [ ] The bounded-counter test extended from single-process to two real node processes

### Stage 3 — delivery and settlement wired, not just typed

- [ ] `soko-delivery`: parse a published rate card and compare routes locally, no quote API
- [ ] `soko-settle`: wire the seam over [patala](https://github.com/vul-os/patala) as one
      implementation, and at least one independent implementation, to prove the seam is genuinely
      provider-agnostic rather than accidentally patala-shaped
- [ ] `soko-node`: a binary that can actually run the above, as a seller and as a buyer

### Stage 4 — the gateway, as its own process

- [ ] `soko-gateway`: render a real storefront from signed catalogue objects
- [ ] Origin isolation enforced in code, not just documented — one process, one origin per store,
      verified by a test that a second store cannot read the first store's session
- [ ] Subdomain and custom-domain binding

### Stage 5 — trust and jurisdiction closing the loop

- [ ] `soko-trust`: purchase attestation tied to a real completed order, not a mocked one
- [ ] `soko-jurisdiction`: scope intersection driving an actual fail-closed checkout decision inside
      `soko-node`, not just the standalone type-level check

### Conformance

- [ ] A conformance vector set, so a second implementation can be checked against Soko's behaviour
      without reading Soko's code — the same neutrality TRACT's own governance requires of the spec
      applies to what "conformant" means in practice
- [ ] First tagged release, once there is something a seller could actually run

## Deliberately not planned

Carried forward from [docs/roadmap.md](docs/roadmap.md) because it bears repeating where
implementation decisions get made, not just where the spec does:

- **A token.** Not now, not later.
- **A hosted marketplace operated by us.** Gateways are meant to be many and competing; one run by
  this project's authors would distort exactly what the design is supposed to prove.
- **A canonical ranking service.** It would be the authority the design removes. Indexes are meant
  to disagree.
- **Cross-seller distributed transactions.** A multi-seller cart stays a set of independent orders
  with compensating actions, stated honestly in the interface — not a distributed transaction,
  which would need a coordinator with authority over sovereign parties.

## What would change this sequence

The stage order assumes the DMTAP substrate crates (`dmtap-core`, `dmtap-sync`) land in roughly the
shape `Cargo.toml`'s commented-out placeholders expect. If the substrate's shape changes materially,
Stage 1 changes with it — Soko is meant to be byte-interoperable with the substrate's reference
implementation, not an independent reinvention that drifts from it.
