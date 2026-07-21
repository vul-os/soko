# Gateways

A **gateway** is the one operator class in TRACT. It exists because two things cannot be
reciprocally provisioned from a keypair: rendering a store to a browser that cannot verify
signatures, and holding money for strangers.

## What it does

- **Storefront** — renders signed catalogue objects into ordinary HTML over ordinary HTTPS, so a
  shopper with no client and no keypair can browse and buy.
- **Settlement and escrow** — holds funds between order and delivery under its own
  payment-provider relationship, and rules on release or refund. See [Settlement](./settlement.md).

These bundle because the same commercial and legal standing underwrites both, and because it
matches how the world already works: a hosted storefront and its payment offering are typically the
same business.

## What makes it different from a platform

| | Hosted platform | TRACT gateway |
|---|---|---|
| Storefront rendering | yes | yes |
| Payments / escrow | yes | yes (its own provider relationship) |
| Owns your catalogue | **yes** | no — it is your signed feed |
| Owns your customer list | **yes** | no |
| Can suspend you | **yes** | can stop serving you; your store still exists |
| Holds your identity keys | n/a | **never** |
| Cost of leaving | export and rebuild | a DNS change |

A seller may list through several gateways at once. The feed is the same object; the gateway is a
renderer, not an owner.

## Domains

Both modes, as a hosted platform would offer:

- **Subdomain** — `<store>.<gateway-domain>`, from one wildcard certificate. Instant, works from a
  bare key with no domain of your own.
- **Custom domain** — point your own domain at the gateway; it provisions TLS. Portable, because
  identity is the keypair and the store is the feed — repointing later changes neither.

### Origin isolation is mandatory

Merchant-supplied render bundles are untrusted code. If stores shared an origin, one malicious
bundle could read another store's cart and session. **Every store gets its own origin** —
single-label subdomains or a custom domain — and a gateway that serves multiple stores from one
origin is non-conformant, not merely ill-advised.

## The honest limit

TRACT's gateway is structurally different from DMTAP's mail gateway in one respect, and the
specification says so rather than letting it be discovered:

**DMTAP's gateway self-extinguishes** — its value to a user shrinks as more of their
correspondents adopt the protocol, until nobody needs it.

**TRACT's storefront gateway does not.** Browsers are permanent. A shopper without a keypair cannot
verify a signature, so they are trusting the gateway to have rendered honestly. That is a real
trust downgrade.

What bounds it, without removing it:

- the gateway serves objects it can itself verify, and every one is independently verifiable;
- **any node can re-render the same store from the same signed objects** and be compared
  byte-for-byte, so dishonest rendering is detectable by anyone who looks;
- a TRACT-native client verifies everything itself and needs no gateway at all;
- two native parties never need a gateway to transact.

The mitigation is detectability, not prevention. Stating it that way is the point.
