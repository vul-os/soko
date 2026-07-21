# Trust & reviews

## Why there is no global star rating

A single canonical score requires a party that aggregates and ranks. That party is an authority: it
decides what counts, who is visible, and who is buried. It is the marketplace, reintroduced under a
different name.

So TRACT forbids a network-wide published score. What it provides instead is better in one specific
way and worse in another, and both should be stated.

## Reviews are signed objects, proven against purchases

A review is an ordinary signed public object attached to a product address, a seller, a
distributor, or a courier. What makes it hard to game is the **purchase attestation**: a proof
that the author actually transacted, issued by the seller or by an escrow operator at completion.

This is stronger than a platform's "verified purchase" badge, because it is verifiable by anyone
rather than asserted by the platform. And it puts a floor under Sybil attacks: **ballot-stuffing
requires actually buying the thing**, at real cost, repeatedly.

It does not eliminate manipulation, and the failure mode is measured rather than hypothetical.
OpenBazaar — self-published unweighted reviews, no banning authority, opt-in escrow — had **one
vendor fake 60% of all measured sales value**. Purchase attestation is strictly stronger than what
OpenBazaar had, but two of its failure conditions survive:

1. **Self-dealing produces genuine attestations.** A seller transacting with itself generates real
   proofs. Attestation raises the cost; it does not establish that the counterparty was independent.
2. **Opt-in escrow is declined by exactly the actors it constrains.** Escrow here is per-order and
   optional, for a good reason — mandatory escrow would exclude regions no licensed operator
   serves. But the measured consequence of optionality is that it goes unused where it matters
   most. Both of those cannot be true for free, and this design pays on the second.

What the achievable Sybil-cost floor is on a signed-feed substrate is genuinely unknown; it has not
been researched here, and claiming otherwise would be inventing a result.

## Ranking is derived

Any node may compute scores over the reviews it has. Indexes will weight differently — recency,
escrow-completed-only, category, or web-of-trust distance from *you specifically*. A seller who
dislikes one index's ranking uses or builds another.

The honest cost: **rankings will disagree between indexes.** There is no single number everyone
sees. For a buyer used to one authoritative rating, that is a worse experience. It is the direct
price of there being no authority, and the documentation says so rather than implying parity.

## Local measurement, not published scores

The same rule the substrate applies to infrastructure applies here: each participant routes by its
**own** measured experience. A courier that misses transit estimates loses *your* traffic based on
*your* history with it. This is automatic, needs no adjudicator, and cannot be gamed centrally
because there is no centre to game.

## Reviews and the right to erasure

This is the one genuinely uncomfortable corner, and it is disclosed rather than hidden.

Published objects are content-addressed and irrevocable. A review is public by nature and is signed
by a person — and under GDPR, POPIA and LGPD a pseudonymous key that is linkable to a person is
still personal data. So "delete my review" cannot be fully satisfied at the protocol layer.

What TRACT does:

- reviews are signed with a **per-seller pseudonymous subkey**, not the author's root identity, so
  a review is not trivially linkable across sellers;
- retraction is a **superseding tombstone**, and conformant clients and gateways honour it by
  ceasing to display and ceasing to serve;
- the residual is stated: **if any independent holder retains the bytes, they persist.**

This is why nothing else personal — no name, no address, no order content — is ever published.
Reviews are the deliberate, bounded exception, not the general rule.
