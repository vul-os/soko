# Roadmap

> Written as sequence, not dates. Anything with a date attached this early would be fiction.

## Now — specification

TRACT is being written before the implementation, deliberately. The failure mode being avoided is a
protocol that is a description of whatever the first implementation happened to do.

- [x] Architecture, roles, the single operator class, the public/sealed split
- [ ] §2 Catalogue — product records, identity ladder, variants
- [ ] §3–§5 The four axes — availability, fulfilment, consideration
- [ ] §6–§7 Cart and order — bounded-counter inventory, sealed order state machine
- [ ] §8 Delivery — rate cards, legs, consolidation
- [ ] §9 Settlement — the seam, rail classes, escrow scope declarations
- [ ] §10–§11 Trust and jurisdiction
- [ ] §12–§13 Gateway and analytics
- [ ] §15–§19 Conformance, wire format, errors, state machines, parameters
- [ ] Conformance vectors — the cross-implementation proof

## Next — the spine

- [ ] `soko-catalog` — publish and verify a catalogue feed end to end
- [ ] `soko-order` — buyer-side cart CRDT; sealed orders between two nodes
- [ ] Bounded-counter inventory across replicas, with a partition test that proves no oversell
- [ ] `soko-delivery` — rate-card parsing and local route comparison
- [ ] `soko-node` — the binary that ties them together

## Then — the edges

- [ ] `soko-gateway` — storefront rendering, per-store origin isolation, custom domains
- [ ] `soko-settle` — seam wiring over patala and at least one non-patala implementation, to prove
      the seam is genuinely provider-agnostic
- [ ] `soko-trust` — purchase attestations and local ranking
- [ ] `soko-jurisdiction` — scope intersection and fail-closed checkout

## Deliberately not planned

- **A token.** Not now, not later.
- **A canonical ranking service.** It would be the authority the design removes.
- **A hosted marketplace operated by us.** Gateways are meant to be many and competing; one run by
  the spec's authors would distort exactly what it is supposed to prove.
- **Cross-seller distributed transactions.** They need a coordinator with authority over sovereign
  parties. Compensating actions instead, stated honestly in the interface.

## Open questions

Genuinely unresolved, listed rather than hidden:

- **Near-duplicate product records.** The content-address floor is exact-match; entity resolution
  across almost-identical records is an index-side heuristic, and heuristics differ between indexes.
- **Bootstrapping manufacturer signatures.** The top rung of the identity ladder needs brands to
  participate, and most will not, early.
- **Aggregate analytics at low volume.** Privacy-preserving aggregation needs volume; small sellers
  get noisy numbers, and it is not clear how much that matters in practice.
- **Distributor insurance.** Signed custody handoff proves transfer but does not make loss
  recoverable. Whether an insurance layer belongs in the protocol, in a profile, or nowhere is open.
