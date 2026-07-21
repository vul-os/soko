# Analytics

Merchants need funnels, attribution and fraud signals. The substrate this is built on spends its
entire design budget hiding exactly the data those are usually derived from. That tension is real
and TRACT resolves it deliberately rather than by accident.

## The posture: tiered, buyer-granted, with an aggregate floor

| Stage | What the merchant gets | Why |
|---|---|---|
| **Browse** | anonymous by default | catalogue reads are pulls of public objects; there is no session to attach identity to |
| **Granted** | scoped, signed disclosure the buyer's node chooses to attach — coarse geography, referrer, session continuity | the buyer decides, per store, and can revoke |
| **Order** | full detail: name, address, contact, fraud-relevant signals | the merchant needs it to fulfil, and the buyer knowingly provides it |
| **Aggregate** | counts, funnels, coarse geography across *all* visitors | derived from opt-in telemetry that carries no per-visitor record |

Raw per-visitor IP logging is **not** the default. A merchant running their own node and serving
their own storefront sees their own server logs, as any web server operator does; what TRACT
declines to do is make cross-store visitor tracking a protocol feature.

## What a merchant genuinely loses

Stated plainly, because a page that claims parity would be lying:

- **No cross-site retargeting.** There is no shared identifier to follow a person between stores.
- **Weaker attribution.** Aggregate attribution gives you campaign-level truth, not
  person-level journeys. Small-volume merchants get noisier numbers, because aggregation needs
  volume to be both private and accurate.
- **No individual visitor replay.** You cannot watch one person's path through your store.
- **Fraud signals are coarser.** See below.

What a merchant keeps: conversion funnels, traffic sources, geography sufficient for shipping and
tax, product-level performance, and full order data for customers who actually bought.

## Fraud without raw IP

Most of what IP is used for in fraud scoring is a proxy for three things: *is this a bot*, *is this
the same actor as last time*, and *is this location consistent with the payment method*.

- **Bot / rate abuse** — anonymous rate-limiting credentials answer this without identity: a
  visitor proves they are within a rate budget without revealing who they are.
- **Repeat-actor detection** — a per-store pseudonymous identifier gives linkage within a store
  without linkage across stores.
- **Payment-location consistency** — this is the payment provider's job, and providers already do
  it with data they hold anyway. A reversible rail brings its own fraud apparatus.

The residual is honest: a merchant on a final, non-reversible rail with no provider-side fraud
tooling has weaker protection than one behind a card network. That is a reason to choose the rail
class deliberately, which is why rail class is in the type.

## Why not just log everything

Because the wedge of this entire design is that the buyer's data is the buyer's. Shipping a
surveillance layer to reach feature parity with hosted platforms would trade away the only durable
reason to prefer it — and the fraud argument usually offered for it turns out to be answerable by
other means.
