<div align="center">

# Soko

### The reference implementation of [TRACT](https://github.com/vul-os/tract)

**Commerce without a marketplace.** A keypair is a store. A cart is yours. Delivery is computed,
not brokered.

*Soko — Swahili, "market."*

</div>

---

## What this is

[TRACT](https://github.com/vul-os/tract) — *Trade, Routing, Attestation, Custody & Trust* — is an
open protocol for decentralized commerce: goods, services, rentals, subscriptions, and the
delivery and settlement around them, between self-sovereign identities with **no marketplace
operator**.

**Soko is one implementation of it.** It is not the standard and it is not required to speak it;
independent implementations MUST be buildable from the TRACT spec alone. Where Soko and the spec
disagree, the spec wins.

## The shape, in one table

| | Centralized platform | TRACT / Soko |
|---|---|---|
| Your catalogue | rows in the platform's database | a signed feed you publish |
| Your customers | the platform's list | your own records, sealed |
| Product identity | the platform issues an ID | content address — two sellers of the same record converge by construction |
| Who can delist you | the platform | nobody |
| Your cart | a session on their server | CRDT state on your own devices |
| Search / ranking | the platform decides | derived index, anyone may build one, none authoritative |
| Leaving | export and rebuild | change a DNS record |

## What Soko does *not* remove

Two things stay real, and the spec says so rather than discovering it later:

- **A shopper with no keypair must trust something to render a store.** That is the storefront
  gateway (TRACT §12), and unlike DMTAP's SMTP gateway it does not self-extinguish, because
  browsers are permanent.
- **Holding money for strangers is a licensed activity.** Escrow is an operator class (TRACT
  §9.6) — permissionless to enter, competing, chosen per-order by both parties, never in
  possession of anyone's identity keys, but an operator class nonetheless.

Everything else is a role any node may take.

## Status

**Pre-alpha. Nothing here is usable yet.** The spec is being written first, on purpose — see
[tract](https://github.com/vul-os/tract). Crates land as their section settles; the workspace
lists only what exists, so an empty `members` entry never implies coverage.

| Crate | TRACT § | Status |
|-------|---------|--------|
| `soko-core` | §16 wire format | scaffold |
| `soko-seam` | §9 settlement, §12 gateway | scaffold |
| everything else | — | see `Cargo.toml` for the planned order |

## Substrate

Soko does not reimplement identity, feeds, blobs, sync, or reachability. It adopts the **DMTAP
substrate** under that directory's à-la-carte rule — *if you implement a capability's function,
you speak its spec* — and adds only the commerce spine.

```
DMTAP substrate:  Identity · Feeds & Blobs · Sync · Infra Roles · Wake
                                   |
TRACT (this):     catalogue · offer · cart · order · delivery · settlement · trust
```

Settlement rides a provider-agnostic seam. [`patala`](https://github.com/vul-os/patala) is *one*
implementation of that seam; `soko-seam` names no payment provider anywhere in its traits, and an
operator wiring Stripe, Peach, M-Pesa, or their own ledger never depends on patala at all.

## History

This repository begins on the commit history of **cartcrft**, its direct predecessor: a
centralized multi-tenant commerce platform. None of that code carries over — Soko is Rust on a
signed-object substrate, not TypeScript on a shared database — but the history does, because the
surface area cartcrft had to cover (bookings, subscriptions, B2B price lists, returns, loyalty,
3PL, duties, tax, channel sync) is the requirements list TRACT has to answer. `.claude/` and env
paths were purged from the rewritten history before grafting; commit hashes therefore differ from
the original repository, which remains the authority for its own history.

## Licence

MIT — so any party, including competitors, may embed it. The TRACT specification is licensed
separately under CC BY 4.0.
