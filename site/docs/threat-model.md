# Threat model & honest limits

A decentralized design that hides its operator classes is lying about them. This page collects what
TRACT cannot do, in one place, without softening.

## What the field evidence says

Before the list of limits: the closest deployed relative of this design **failed**, and the largest
live one **succeeded by adopting an operator**. Both belong here.

**OpenBazaar** (signed objects, content-addressed listings, keypair identity, no operator; shut
down January 2021):

| Measure | Result |
|---|---|
| Lifetime participants | ~6,651 |
| Concurrently online | ~80 |
| Credible sales over 14 months | ~US$86,000 |
| Median listing lifetime | ~22 days |
| Share of measured sales value faked by one vendor | **60%** |

Its four failure modes: negligible activity; **discovery re-centralized first** (the default
crawler became a content-policy gatekeeper); **availability bounded by publisher liveness**
(catalogues vanished when nodes left); and **reputation trivially ballot-stuffed** with opt-in
escrow declined by exactly the actors it existed to constrain.

**Beckn / ONDC** — the largest live decentralized-commerce network — avoids all four by doing the
opposite of this design: a central registry that gates keys, approval-gated enrollment with
whitelisting and probation, identity anchored to DNS/TLS rather than keypairs, and rate-limited
operator-hosted discovery.

The honest reading: **the network with volume chose an operator at exactly the three points this
design tries to leave operator-free.** That is not proof this fails. It does mean the burden of
proof sits here.

*Caveats: the OpenBazaar figures come from a single peer-reviewed measurement study; sales are an
acknowledged lower bound from voluntary feedback, and listing-lifetime conflates deliberate
delisting with liveness eviction. Beckn docs and endpoints have changed since.*

## Operator classes

TRACT has **one**: the gateway. DMTAP has one too, but DMTAP's self-extinguishes as adoption grows
and TRACT's does not, for two reasons:

1. **Browsers are permanent.** A shopper without a keypair cannot verify a signature, so someone
   must render honestly on their behalf. Mitigated by universal re-renderability — any node can
   produce the same page from the same signed objects and be compared — but not removed.
2. **Holding money for strangers is licensed activity.** Escrow cannot be reciprocally provisioned.

What is preserved: the class is one, entered permissionlessly, competing, chosen per-order by both
parties, replaceable without loss of catalogue or customers, and **never in possession of identity
keys**.

## Irrevocability versus erasure rights

Published objects are content-addressed and irrevocable. Data-protection erasure rights cannot be
satisfied against them.

**Resolution:** no personal data enters the public quadrant. Orders, addresses and contact details
are sealed and deletable at the edges. The bounded exception is reviews — public by nature, signed
by a person — handled with per-seller pseudonymous subkeys, superseding tombstones honoured by
conformant clients and gateways, and an explicit residual: **if any independent holder keeps the
bytes, they persist.**

## Reputation

- **No global score exists**, so buyers lose the convenience of one authoritative number, and
  indexes will disagree. This is the direct price of removing the authority.
- **Purchase attestation raises the cost of manipulation but does not eliminate it.** A seller can
  transact with itself. The attestation makes that expensive and leaves a public trail.
- **Whitewashing is bounded but not solved.** A new key has no history; buyers weighting history
  will discount it, which is the intended and only available defence.

## Availability

- **A public object is available exactly as long as some holder serves it.** Content addressing is
  a name, not a durability promise.
- **A seller's node being offline** delays orders but does not lose them — the sender's node
  retries. **It does lose the catalogue.** An offline seller is invisible, not slow, and nobody is
  obliged to serve their listings in their absence. This was OpenBazaar's measured failure, and
  unpaid third-party replication is exactly what it did not attract. Whether pinning needs an
  incentive — and whether that incentive creates another operator — is unresolved.
- **Stranded inventory quota.** A partitioned replica holding unsold quota strands that stock until
  it rejoins.

## Transactional integrity

There is **no cross-seller atomicity**. A multi-seller cart is a set of independent orders;
checkout is modelled as compensating actions, never a distributed transaction, because a
distributed transaction across sovereign parties needs a coordinator with authority over all of
them. Interfaces must show per-seller status honestly rather than a single "order placed".

## Physical custody

Cannot be made trustless. A distributor holds goods belonging to strangers. Signed handoff
attestations make custody transfer *provable*; they do not make loss or damage *recoverable*.
Non-custodial programmatic escrow removes the custodian but deadlocks on genuine disputes — the
exact case it was wanted for.

## Privacy of reads

Public catalogue reads are anonymous to the object but not to the transport: whoever serves you
bytes learns which bytes you asked for. TRACT does not route public fetches through a mixnet,
because the objects carry no secret. A reader who needs to hide *that they browsed* must supply
their own transport anonymity.

## Analytics

Merchants lose cross-site retargeting, person-level attribution, and individual session replay.
Fraud signals are coarser on non-reversible rails. See [Analytics](./analytics.md).

## Legal

The protocol makes the responsible party explicit, signed and attributable. It does not make any
deployment compliant. Operators need their own counsel — particularly for escrow, which is licensed
activity almost everywhere.
