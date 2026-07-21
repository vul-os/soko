# Architecture

> **Pre-alpha.** The workspace lists only crates that exist. Planned crates are a comment in
> `Cargo.toml` with the TRACT section each answers — an empty crate per section would claim
> coverage that is not there.

## Language and why

Rust, because the substrate's reference implementation is Rust (`dmtap-core`, `dmtap-sync`) and
Soko must be byte-interoperable with it rather than a second implementation of the same objects
that drifts.

## Dependency direction, fixed from the first commit

```
soko-seam  (zero dependencies)
    ^
    |
soko-core  (objects, wire format)
    ^
    |
everything else
```

`soko-seam` depends on nothing and must stay that way — `cargo tree -p soko-seam` is one line. A
dependency added there is inherited by every implementor of the seam, which is exactly what a seam
exists to prevent.

## Crates

| Crate | TRACT § | Status | Purpose |
|---|---|---|---|
| `soko-seam` | §9, §12 | scaffold | settlement, escrow and storefront traits that name no provider |
| `soko-core` | §16 | scaffold | object model, split `public` / `sealed` |
| `soko-catalog` | §2 | planned | product records, offers, variants, the identity ladder |
| `soko-availability` | §3 | planned | stock, slots, capacity, made-to-order |
| `soko-order` | §6–§7 | planned | buyer-side cart CRDT, sealed orders, state machine |
| `soko-delivery` | §8 | planned | rate cards, legs, consolidation, local routing |
| `soko-settle` | §9 | planned | payment seam wiring; escrow attestations |
| `soko-trust` | §10 | planned | purchase-attested reviews, local ranking |
| `soko-jurisdiction` | §11 | planned | the four anchors, scope declarations |
| `soko-node` | — | planned | the node binary |
| `soko-gateway` | §12 | planned | storefront gateway — **separate process** |

## The public/sealed split is structural

`soko-core` separates `public` and `sealed` into distinct modules rather than one flat namespace,
because the rule they encode is easy to violate by accident:

- **public** — signed, content-addressed, globally deduplicated, **irrevocable**. Products, offers,
  rate cards, reviews. A type here must be structurally incapable of carrying a name or an address.
- **sealed** — encrypted to the counterparties, never published, deletable at the edges. Orders,
  addresses, contact details, payment references.

A right to erasure cannot be satisfied against an irrevocable content-addressed object. Keeping the
two in separate modules is cheaper than remembering the rule at every call site.

## What Soko does not implement

Identity, signing, content addressing, feeds, blobs, sync and reachability are the **DMTAP
substrate's** job. Soko adopts them. A hash construction or signature framing invented inside
`soko-core` is a bug, not a feature.

Settlement rails are **patala's** job, or any other implementation of the seam. Soko wires the
seam; it does not ship a rail.

## Gateway process isolation

The gateway terminates untrusted connections and renders untrusted merchant bundles. It runs as a
separate process with no access to identity keys or the object store, and serves every store from
its own origin. "One binary, several roles" never means one address space.

## History

This repository begins on the commit history of **cartcrft**, a centralized multi-tenant commerce
platform that was its direct predecessor. None of that code carries over. The history does, because
the surface area cartcrft had to cover — bookings, subscriptions, B2B price lists, returns, loyalty,
third-party logistics, duties, tax, channel sync — is the requirements list TRACT has to answer.
What it could not do is any of it without a central database holding every tenant's rows behind
row-level security, which is the one assumption TRACT removes.

Commit hashes differ from the original repository: `.claude/` and environment paths were purged
from the whole history with `git-filter-repo` before grafting, which rewrites every hash downstream
of the first change. The content is otherwise intact, so this repository is a complete record of
that history rather than a pointer to one.
