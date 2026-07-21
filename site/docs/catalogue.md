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
