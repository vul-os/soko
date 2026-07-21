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

```
                sellers                 discovery              buyer
        ┌──────────────────────┐   ┌───────────────┐   ┌──────────────────┐
        │  Seller A  ──┐       │   │               │   │                  │
        │  Seller B  ──┼──────────▶│  any index    │──▶│   one cart       │
        │  Seller C  ──┘       │   │  derived,     │   │   routing local  │
        │  signed feeds        │   │  rebuildable  │   │   sealed orders  │
        └──────────────────────┘   └───────────────┘   └──────────────────┘
              ▲                                                  │
              └──────────────  sealed order per seller  ─────────┘
```

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

| Crate | TRACT § | Status |
|---|---|---|
| `soko-seam` | §9, §12 | scaffold — settlement/escrow/storefront traits that name no provider |
| `soko-core` | §16 | scaffold — object model, `public` / `sealed` split |
| everything else | — | see [`Cargo.toml`](Cargo.toml) for the planned order |

```sh
cargo check --workspace
cargo tree -p soko-seam   # must stay one line — a dep here is inherited by every implementor
```

## Architecture

```
DMTAP substrate:  Identity · Feeds & Blobs · Sync · Infra Roles · Wake
                                   │
TRACT:            catalogue · offer · cart · order · delivery · settlement · trust
                                   │
Soko:             this repository
```

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

## History

This repository begins on the commit history of **cartcrft**, its direct predecessor: a centralized
multi-tenant commerce platform. None of that code carries over — Soko is Rust on a signed-object
substrate, not TypeScript on a shared database — but the history does, because the surface area
cartcrft had to cover (bookings, subscriptions, B2B price lists, returns, loyalty, 3PL, duties, tax,
channel sync) is the requirements list TRACT has to answer.

`.claude/` and environment paths were purged from the entire history with `git-filter-repo` before
grafting, which rewrites every downstream hash. The content is intact, so this repository is a
complete record of that history rather than a pointer to one.

## Licence

MIT — so any party, including competitors, may embed it. The TRACT specification is licensed
separately under CC BY 4.0.
