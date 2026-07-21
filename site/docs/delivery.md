# Delivery & routing

## Rate cards are published, not quoted

A courier — a national carrier, a distributor, or someone with a van — publishes a **rate card** as
a signed public object: zone table, weight brackets, dimensional divisor, surcharges, transit
estimates, and the countries it serves.

The buyer's node then **computes** prices locally instead of calling a quote API. That has
consequences beyond convenience:

- no rate-limit, no API key, no per-quote cost;
- no third party learns what you are shopping for, or from whom;
- comparison across every courier is possible without a broker;
- a peer courier and a multinational are the same object type, so there is no second-class tier.

Because the substrate deduplicates identical public blobs globally, one carrier's rate card is
stored once no matter how many sellers reference it.

> **Caveat, stated plainly:** some carriers restrict redistribution of negotiated rates in their
> API terms. Published list rates and a seller's own negotiated rates that the seller chooses to
> publish are fine; republishing a carrier's confidential rates is the seller's compliance problem,
> not something the protocol can wave away. Where a live quote is genuinely required, an offer may
> declare that and the buyer's node will request one rather than computing.

## Legs and consolidation

A **leg** is one movement between two places, priced by one rate card. A **consignment** is goods
in someone else's custody — a courier leg or a distributor hold.

For a multi-seller cart the buyer's node compares a small number of concrete routings:

| Option | Shape | Wins when |
|---|---|---|
| direct | each seller ships to the buyer | sellers are far apart, or speed matters most |
| hub near buyer | all sellers ship to a distributor local to the buyer, who consolidates | last-mile dominates cost |
| hub near sellers | sellers cluster geographically; consolidate first, then one long leg | the long haul dominates |

The comparison is `Σ leg costs + storage_fee × wait_days + handling`, over a handful of candidate
hubs — small enough to evaluate exhaustively rather than needing an optimiser. The genuinely hard
input is `wait_days`: when the slowest seller's parcel actually arrives. That is estimated from
published transit times and stated as an estimate, never as a promise.

## Distributors

A distributor is another node publishing capacity — space, location, per-day storage rate, handling
fee, and the categories it will not take. It receives consignments, holds them, consolidates, and
either hands off to a courier or delivers locally.

Economically this is the freight-forwarder/consolidator pattern that already exists worldwide;
what TRACT adds is that entry is permissionless and the terms are a published object rather than a
contract negotiated per relationship.

**The custody problem is real.** A distributor holds goods belonging to people who have never met
them. TRACT does not pretend a protocol solves this:

- a distributor's acceptance and handoff are signed attestations, so custody transfer is provable;
- disputes over goods in custody route to whatever escrow arrangement the parties chose;
- where no escrow was chosen, the buyer bears the risk, and the interface must say so **before**
  the order, not after.

## What TRACT does not do

It moves nothing, insures nothing, and guarantees no delivery date. It computes over published
claims. A rate card is a claim by its publisher; a transit estimate is an estimate; and a courier
that misses them accumulates a locally-measured reputation with the parties who used it — never a
globally published score.
