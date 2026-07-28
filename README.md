<p align="center">
  <img src="assets/logo.svg" width="112" alt="Soko" />
</p>

<h1 align="center">Soko</h1>

<p align="center">
  <strong>Commerce without a marketplace. A keypair is a store.</strong><br />
  <sub>Reference implementation of <a href="https://github.com/vul-os/tract">TRACT</a> — an open protocol for decentralized commerce. Rust. MIT.</sub>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-16D97F?style=flat-square&labelColor=07140E" alt="MIT License" /></a>
  <a href="https://github.com/vul-os/tract"><img src="https://img.shields.io/badge/protocol-TRACT%200.1.0-16D97F?style=flat-square&labelColor=07140E" alt="TRACT" /></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.80%2B-16D97F?style=flat-square&labelColor=07140E" alt="Rust" /></a>
  <a href="docs/threat-model.md"><img src="https://img.shields.io/badge/operator%20classes-1-E9A23B?style=flat-square&labelColor=07140E" alt="One operator class" /></a>
  <a href="#status"><img src="https://img.shields.io/badge/status-pre--alpha-A1A1AA?style=flat-square&labelColor=07140E" alt="Pre-alpha" /></a>
</p>

<p align="center">
  <a href="docs/overview.md">Overview</a> ·
  <a href="docs/catalogue.md">Catalogue</a> ·
  <a href="docs/offers.md">The four axes</a> ·
  <a href="docs/delivery.md">Delivery</a> ·
  <a href="docs/threat-model.md">Honest limits</a> ·
  <a href="docs">Docs</a>
</p>

---

A commerce platform welds together four things that need not be joined: **who you are** (an account
it issues), **what you sell** (rows in its database), **who can find you** (its ranking), and **how
you get paid** (its payment relationship). Lose the account, lose all four.

Soko separates them. Your catalogue is a signed feed you publish. Your cart is CRDT state on your
own devices. Delivery is computed locally from published rate cards rather than brokered. Nobody
can delist you, and leaving costs a DNS change.

<p align="center">
  <img src="docs/diagrams/cart-flow.png" width="760" alt="One cart across independent sellers" />
</p>

No party sees the whole cart. The index holds no authority — a disagreement between an index and a
seller's feed always resolves in favour of the feed.

---

## What changes when there is no operator

| | Centralized platform | Soko |
|---|---|---|
| Your catalogue | rows in their database | a signed feed you publish |
| Product identity | the platform issues an ID | a content address |
| Who describes the product | the platform | the manufacturer, by signature |
| Your cart | a session on their server | CRDT state on your devices |
| One cart across sellers | impossible — separate checkouts | native |
| Ranking | the platform decides | derived; build your own index |
| Who can delist you | the platform | nobody |
| Leaving | export and rebuild | change a DNS record |

## One shape for every trade

Goods, services, rentals, bookings and subscriptions are the same object with four axes — not a
plugin per category.

| Axis | Variants |
|---|---|
| **Item** | product · variant-of-group · service · right/licence · capacity |
| **Availability** | count · time slots (RFC 5545) · capacity per interval · unlimited · made-to-order |
| **Fulfilment** | ship · collect · digital grant · perform-at-place · perform-remote · access grant · return-required |
| **Consideration** | fixed · tiered · recurring · metered · deposit+balance · quote/RFQ |

A rental is `product + time-slots + ship/return-required + fixed-per-period + deposit`. A restaurant
booking is `capacity + capacity-per-interval + perform-at-place`. A metered API is
`right + unlimited + access-grant + metered`. None needs a category-specific code path.

## What Soko does **not** remove

A decentralized design that hides its operator classes is lying about them.

- **Somebody renders the store.** A shopper without a keypair cannot verify a signature, so they
  trust a storefront gateway. Unlike DMTAP's mail gateway this never self-extinguishes — browsers
  are permanent. Mitigated by universal re-renderability, not removed.
- **Somebody holds the money.** Escrow is a licensed activity. It is an operator class:
  permissionless to enter, competing, chosen per order by both parties, scoped to the jurisdictions
  it is actually licensed for, and never in possession of identity keys.
- **Nobody agrees on a star rating.** Ranking is derived, so indexes will disagree. There is no
  canonical 4.7 stars, because computing one requires the authority being removed.

## A store, rendered

<p align="center">
  <img src="docs/screenshots/storefront.png" width="820" alt="A Soko storefront: six listings — a notebook, a workshop, a scaffold hire, a font licence, a made-to-measure apron and a bulk quote — all the same offer object with different axis values" />
</p>
<p align="center"><em>Six listings, one object model. A notebook, a workshop, a scaffold hire, a font licence, a made-to-measure apron and a bulk quote &mdash; no product type, no booking module, no rentals plugin. Note the workshop&rsquo;s place of supply is <strong>DE</strong> while neither party is German: it comes from the fulfilment axis, not from either party&rsquo;s country.</em></p>

```sh
cargo run -p soko-gateway --bin soko-storefront -- --serve 8080
```

<sub>Rendered by <code>soko-storefront</code> from real catalogue objects &mdash; the axis lines, the routing comparison and the review weighting are live calls into the workspace crates. <strong>Not yet a gateway:</strong> it does not fetch from a feed, verify a signature, or take an order. Regenerate with <code>node tools/screenshots.mjs</code>.</sub>

## Diagrams

<p align="center">
  <img src="docs/diagrams/axes.png" width="800" alt="The four axes of an offer" />
</p>
<p align="center"><em>Every trade is one object with four axes. The dotted edge is the one that catches people out: <strong>fulfilment also derives place of supply</strong>, which is the tax anchor.</em></p>

<p align="center">
  <img src="docs/diagrams/checkout.png" width="800" alt="Checkout across independent sellers" />
</p>
<p align="center"><em>The buyer's node orchestrates. The gateway appears <strong>only</strong> if both parties chose escrow — two native parties never need one.</em></p>

<p align="center">
  <img src="docs/diagrams/routing.png" width="560" alt="Direct versus hub consolidation" />
</p>
<p align="center"><em>Rate cards are published, so consolidation is compared locally. No quote API, and nobody learns the cart.</em></p>

<sub>Regenerated from one source with <code>node tools/diagrams.mjs</code>. See <a href="docs/diagrams.md">Diagrams</a>.</sub>

## Read the evidence before believing any of it

[TRACT §21](https://github.com/vul-os/tract/blob/main/21-grounding.md) records an
adversarially-verified literature pass **including the findings that contradict the design**:

- **OpenBazaar** — the closest deployed relative — shut down in 2021 having moved ~**$86k over 14
  months**, ~80 users online at a time, **one vendor faking 60% of measured sales value**. Discovery
  re-centralized first. Catalogues vanished when nodes went offline. Opt-in escrow was declined by
  exactly the actors it existed to constrain.
- **Beckn/ONDC**, the largest live decentralized-commerce network, avoids all of that by adopting a
  **central approval-gating registry** — the opposite of this design, at its three weakest points.
- **There is no deployed permissionless global product identity.** The space between GS1's licensed
  monopoly namespace and a nominal merchant string is currently evidence-free.

Those findings are folded back into the spec rather than footnoted. A specification that omitted
them would be easier to believe and worse to build on.

## Status

**Pre-alpha. Nothing here is usable yet.** The protocol is being written first, on purpose — the
failure mode being avoided is a spec that is a description of whatever the first implementation
happened to do.

| Crate | TRACT § | Covers |
|---|---|---|
| `soko-seam` | §9, §12 | settlement / escrow / storefront traits that name no provider — **zero dependencies** |
| `soko-core` | §16 | content addresses, money, places, and the `public` / `sealed` type split |
| `soko-offer` | §3–§5 | the four axes, and the place-of-supply derivation that falls out of fulfilment |
| `soko-catalog` | §2 | product records, product groups, the identity ladder, canonicalisation |
| `soko-order` | §6–§7 | buyer-held cart, per-seller split, bounded-counter inventory, sealed orders |
| `soko-delivery` | §8 | rate cards, volumetric weight, legs, consignments, **distributors**, route comparison |
| `soko-settle` | §9 | payment attestations, rail classes, **escrow scope with fail-closed intersection** |
| `soko-trust` | §10 | purchase-attested reviews, per-index weighting, local-only scoring |
| `soko-jurisdiction` | §11 | the four anchors, place-of-supply resolution, responsible parties |
| `soko-gateway` | §12 | storefront binding and the origin-isolation rule |
| `soko-node` | — | the node binary |

The whole protocol surface has a home, and **124 tests** cover the parts where getting it wrong is
silent: volumetric weight, currency mismatch, escrow scope intersection, place of supply for an
event held abroad, concurrent replicas not overselling, and unattested reviews not moving a score.

Two of those are an **end-to-end trade across every crate** — deliberately the awkward case, not
the easy one: two sellers who have never heard of each other in one cart, one shipping a physical
good and the other selling admission to an event in a third country, a buyer resident in neither,
a seller running two uncoordinated replicas, and an escrow operator licensed for only one of the
two trades. Unit tests prove a type behaves; that one proves they compose.

```sh
cargo run -p soko-node -- demo
```

prints the same trade with its decisions, including the line that matters most: **both parties are
identical between the two trades — only the place of supply differs, and that alone is enough for
the escrow operator to have to refuse one of them.**

```sh
cargo test --workspace     # 124 tests, 1 ignored (the re-vendoring maintenance action)
cargo tree -p soko-seam    # must stay one line — a dep here is inherited by every implementor
```

Twenty-one of those tests' cases are the TRACT conformance vectors — hand-derived from the
specification text, never exported from this code, and vendored into
[`crates/soko-node/tests/tract-vectors/`](crates/soko-node/tests/tract-vectors/) with a digest over
every file. They run in a bare checkout with no network and no second repository, which is the
whole point: a conformance check that only runs where the spec happens to be sitting next door is a
conformance check that reports `ok` and verified nothing.

## Help wanted

The implementation's gaps are mostly ordinary work. The specification's are not — several are
blocked on expertise this project does not have, and had multiple research passes return nothing
verifiable. They are recorded as unevidenced rather than quietly asserted; see
[TRACT's help-wanted](https://github.com/vul-os/tract/blob/main/docs/HELP-WANTED.md).

**On this repository specifically:**

- **Build something that speaks TRACT without reading this code.** It is the most valuable thing
  anyone could do, and what would help most is not the implementation but a list of every place the
  specification was ambiguous or wrong. The conformance vectors in the tract repo are derived from
  the spec text by hand, so you can check an encoder against the document rather than against Soko.
- **Try to break an invariant.** The two most valuable contributions so far were adversarial: a
  review that found six invariants documented but not structurally enforced, and a conformance pass
  that found the specification contradicting itself. Claims asserted in a doc comment with nothing
  enforcing them are the pattern worth hunting.
- **The parts that are genuinely just work:** networked transport for `soko-feed` (it publishes and
  verifies against a directory today), the order flow over a real connection, and the gateway as a
  service with custom domains.

Ordinary contributions are welcome too — see [CONTRIBUTING.md](CONTRIBUTING.md) for the build,
test and review gates.

## Architecture

<p align="center">
  <img src="docs/diagrams/substrate.png" width="620" alt="Soko sits on TRACT, which sits on the DMTAP substrate" />
</p>

Soko implements no cryptography. Identity, signing, content addressing, feeds and sync come from the
[DMTAP substrate](https://github.com/vul-os/dmtap); a hash construction invented in `soko-core`
would be a bug. Settlement rides a provider-agnostic seam —
[patala](https://github.com/vul-os/patala) is *one* implementation of it, and `soko-seam` names no
provider anywhere.

The `public` / `sealed` module split is structural, not stylistic: published objects are
content-addressed and irrevocable, so a right to erasure cannot be satisfied against them.
**Nothing personal may ever be published** — orders, addresses and contact details are sealed and
deletable at the edges. Keeping the two in separate type families is cheaper than remembering the
rule at every call site.

## Licence

MIT — so any party, including competitors, may embed it. The TRACT specification is licensed
separately under CC BY 4.0.

---

<p align="center">
  <a href="https://vulos.org"><img src="site/assets/vulos-logo.png" alt="vulos" height="20"></a><br>
  <sub><a href="https://vulos.org"><b>vulos</b></a> — open by design</sub>
</p>
