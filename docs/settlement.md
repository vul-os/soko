# Settlement & escrow

## The seam names no provider

TRACT specifies **where** money crosses the boundary and **what** must be verified. It does not
specify a rail, a currency, a token, or a ledger.

`soko-seam` therefore names no payment provider anywhere in its traits — not Stripe, not Paystack,
not Peach, not a chain. This is deliberate: the moment a protocol crate names a provider, every
implementor inherits that provider's jurisdiction, licensing, and politics. `cargo tree -p
soko-seam` is one line and must stay that way.

[Patala](https://github.com/vul-os/patala) is *one* implementation of the seam. An operator wiring
their own PSP writes a small implementation and depends on none of it.

## Rail class is part of the type

The most important thing about a payment method, for commerce, is **what recourse it leaves the
buyer**. So the classification is in the type and is never flattened to a boolean:

| Class | Properties | Who adjudicates a dispute |
|---|---|---|
| `CustodialReversible` | chargebacks possible, KYC, delayed settlement | **the card network** — already exists, not yours to build |
| `NonCustodialFinal` | wallet-to-wallet, final, seconds, nobody custodies | **nobody** — and the buyer must be told before confirming |

This resolves the escrow problem for a large class of trades without building anything: for a
stranger selling physical goods, a reversible rail means the existing chargeback machinery *is* the
dispute system. An implementation must not silently fail a final-rail request over to a reversible
one, or vice versa — the guarantees differ, so the substitution is a decision for the parties.

## Escrow is an operator class

Where the parties want held funds and a ruling, TRACT names the role and confines it rather than
pretending a protocol can dissolve it.

An escrow operator requires **scarce resources**: legal standing, a payment-provider relationship,
a float, and jurisdiction-specific licensing. None of that is derivable from a keypair. That makes
it the one operator class in the protocol.

What keeps it from capturing the network:

- **Permissionless entry, competing.** Anyone meeting the legal bar runs one. No registry admits
  them; they publish a capability feed and are discovered like any other identity.
- **Chosen per order.** The seller declares which escrow providers it accepts; the buyer's node
  intersects with those it trusts. Empty intersection means no escrow — disclosed to both parties,
  who may proceed unescrowed or abandon. Never a silent downgrade.
- **Never holds keys.** It holds funds, in its own provider account or contract. Identity keys stay
  at the edges.
- **Every ruling is a signed public object.** Release, refund and split decisions are published to
  the operator's own feed. An operator that rules unfairly accumulates a permanent, verifiable
  record of having done so — accountability without an adjudicator standing above it.

## Jurisdictional scope is mandatory

Escrow means holding client funds, which is licensed activity in most jurisdictions. An operator
therefore publishes the scope it can actually serve: buyer countries, seller countries, supply
countries, currencies, rail classes, value ceilings, excluded categories, and the authorisations it
claims to hold.

Checkout intersects that scope against the actual trade and **fails closed** if it does not match.
An operator licensed for West Africa is not an option for a European consumer transaction, and the
protocol expresses that rather than discovering it in a regulator's letter.

Truthfulness of the declaration is the operator's liability — and because it is a signed public
object, a false one is durable evidence rather than a deniable claim.

> **Unescrowed is always a valid outcome.** If no operator matches, the trade still happens with
> disclosed risk. Escrow is an option, never a gate. Getting this wrong would block exactly the
> underserved markets this design exists to serve.

## What is not solved

Physical custody cannot be made trustless. Non-custodial programmatic escrow (multi-signature,
hashlock plus timelock) removes the custodian but **deadlocks precisely when there is a genuine
dispute** — the case it was needed for. TRACT supports it as a rail and states the limit; it does
not present it as a solution to disputes.
