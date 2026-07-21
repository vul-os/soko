# Threat model & honest limits

A decentralized design that hides its operator classes is lying about them. This page collects what
TRACT cannot do, in one place, without softening.

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
- **A seller's node being offline** delays orders; it does not lose them, because the sender's node
  retries. But a seller offline indefinitely is a seller whose store stops working.
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
