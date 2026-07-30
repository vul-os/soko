# Overview

**Soko is the reference implementation of [TRACT](https://github.com/vul-os/kotva/tree/main/profiles/tract)** — *Trade,
Routing, Attestation, Custody & Trust* — an open protocol for decentralized commerce.

> **Status: pre-alpha.** The specification is being written first, on purpose. Nothing in this
> repository is usable yet, and this documentation describes the design, not a shipped product.
> Where a page says "will", read it as a design commitment, not a claim about running code.

## The one idea

A conventional commerce platform welds together four things that need not be joined: **who you
are** (an account it issues), **what you sell** (rows in its database), **who can find you** (its
search ranking), and **how you get paid** (its payment relationship). Lose the account, lose all
four.

TRACT separates them:

- **Identity** is a keypair you hold. No platform issues it, so none can revoke it.
- **Catalogue** is a signed, append-only feed you publish. Nobody else can edit or remove it.
- **Discovery** is a derived index anyone may build. None is authoritative; a disagreement between
  an index and a seller's feed always resolves in favour of the feed.
- **Settlement** is a seam, with the provider chosen per order by the parties themselves.

## What this buys that a platform cannot offer

| | Centralized platform | TRACT / Soko |
|---|---|---|
| Your catalogue | rows in their database | a signed feed you publish |
| Product identity | the platform issues an ID | a content address; identical records converge by construction — [but see the limit](./catalogue.md#the-identity-ladder) |
| Who describes the product | the platform | the manufacturer, by signature |
| Who can delist you | the platform | nobody |
| Your cart | a session on their server | CRDT state on your own devices |
| One cart across sellers | impossible — separate checkouts | native |
| Ranking | the platform decides | derived; build your own index |
| Leaving | export and rebuild | change a DNS record |

## What it does not buy

Four things stay true, and the spec states them on its first page rather than burying them:

1. **Cross-publisher product identity is unproven.** Identical bytes converge trivially; two shops
   describing the same shoe do not. No deployed system achieves it without a licensed registry, and
   the canonicalisation rules — not the hashing — are where the real work is. See
   [Catalogue](./catalogue.md).
2. **A shopper without a keypair must trust a gateway to render the store honestly.** Unlike
   DMTAP's mail gateway, this never self-extinguishes, because browsers are permanent.
3. **Escrow is an operator class.** Holding money for strangers is licensed activity; TRACT
   confines that role rather than pretending a protocol can dissolve it.
4. **There is no canonical star rating.** Computing one requires an authority that aggregates and
   ranks, which is the thing being removed.

See the [threat model](./threat-model.md), which collects every operator class and residual in one place.

## Where to read next

- [The TRACT protocol](./protocol.md) — how this relates to the spec and to DMTAP
- [Catalogue & product identity](./catalogue.md) — the "many stores, one SKU" problem
- [Offers: the four axes](./offers.md) — one shape for goods, services, rentals, subscriptions
- [Architecture](./architecture.md) — crates, dependency direction, what is implemented
