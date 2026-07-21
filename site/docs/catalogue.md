# Catalogue & product identity

The hardest problem in decentralized commerce is not publishing a catalogue — it is **agreeing on
what a product is** without a registrar.

## Product ≠ Offer

TRACT splits the two, and on a content-addressed substrate the split does most of the work for
free:

- A **product record** describes *what a thing is*. It is a content-addressed public object. It
  belongs to nobody.
- An **offer** is *one seller's claim* that it will supply that thing on stated terms — price,
  stock, fulfilment, jurisdiction. It lives in that seller's signed feed.

Because the substrate addresses public blobs over **plaintext**, two sellers publishing the same
product record compute the **same content address**, and the swarm stores it once. So "who else
sells this?" is a reverse index from product address to offers — derived data, rebuildable by
anyone, authoritative for nobody.

**No registrar issues product IDs.** The global product list is an emergent consequence of hashing.

> **How strong is this actually? Weaker than it sounds.** Convergence is trivially true for
> identical bytes and says nothing about the real case: two shops describing the same shoe. A 2026
> literature pass found **no deployed system achieving cross-publisher product identity without a
> licensed registry**, and the one candidate for permissionless crawl-derived resolution was
> refuted under adversarial verification. The two models that exist in the field are a permissioned
> monopoly namespace (GS1 GTIN — licensed, fee-bearing, gated by national member organisations) and
> a purely nominal string (schema.org `productGroupID`, no issuer, no uniqueness guarantee).
> Nothing in between is deployed. So the content address is a sound **mechanism** with an
> **unproven** claim resting on it, and the canonicalisation rules — not the hashing — are where
> the work actually is.

## The identity ladder

Byte-identity is brittle: two shops describing the same shoe will not produce identical bytes. So
product identity is a ladder, deliberately the same shape as DMTAP's naming ladder:

| Rung | Mechanism | Authority | Failure mode |
|---|---|---|---|
| **Floor** | content address of the canonicalised record | zero — always works | near-duplicates do not collapse |
| **Middle** | claimed external identifiers (GTIN, MPN, ISBN…) | **none — anyone can claim any value** | claim-squatting; indexes must treat as advisory |
| **Top** | record signed by the **manufacturer's own key** | the brand itself | requires the brand to participate |

The top rung is the interesting one. On a centralized marketplace, *the marketplace* is
authoritative about your product's specifications. Here the manufacturer signs the canonical record
and a reseller's offer references it — so **a reseller can add an offer but cannot misdescribe the
product**. Facts and offers are separated by signature, not by policy.

### On GTIN and GS1

External identifiers are supported as **claims only**, never as the identity. GS1 identifier
issuance is gated and carries recurring fees, so a protocol that *depended* on GTIN would import a
centralization point and a cost barrier — precisely against sellers in the markets this is meant to
serve. A seller with a GTIN may publish it as a join key; a seller without one is not disadvantaged
at the floor rung.

## Variants and SKUs

Variants follow the schema.org vocabulary rather than a bespoke model: a **product group** declares
the axes it varies by (size, colour, material), and each variant is its own content-addressed
record referencing the group. This means existing merchant feeds map in with a translation rather
than a redesign.

Sub-products, bundles and kits are expressed as a record referencing component records — including
components published by *other sellers*, which is how a cross-seller bundle becomes possible at all.

## Indexes are not marketplaces

An index builds search, categories, "related products", and rankings over public feeds. TRACT is
explicit that this is **derived data**:

- any node may build one;
- none is authoritative;
- a disagreement between an index and a feed resolves in favour of the feed;
- there is no protocol mechanism by which an index can delist a seller from the network — only
  from *itself*, and another index will differ.

This is the load-bearing distinction between an index and a marketplace, and the word
"marketplace" is deliberately not used anywhere in the spec for anything TRACT defines.

### But permission is not practice

"Any node may build an index" does not mean many will, and this is the design's weakest point
rather than one of its strengths.

A content-addressed substrate has no global index, so **discovery is the first function to
re-centralize**: whichever index becomes economically dominant becomes a de facto content-policy
gatekeeper no matter what the protocol permits. That is precisely what happened to OpenBazaar, the
closest deployed relative of this design — its default search engine became the gatekeeper. And
Beckn/ONDC, the largest live decentralized-commerce network, avoids the problem only by adopting a
central, approval-gating registry with rate-limited lookup: the opposite of this design, at the
exact point this design is weakest.

Multiple competing indexers with verifiable completeness or censorship proofs is the candidate
answer. It has **no deployed precedent**. See [Threat model](./threat-model.md).
