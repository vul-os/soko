# Jurisdiction & tax

TRACT is designed to be used worldwide — South Africa, the EU, other African markets, New Zealand,
the Americas — which means jurisdiction cannot be an afterthought bolted onto a platform's terms of
service. It is a machine-readable field on every offer and order.

## The rule: responsibility follows the money

Every regime asks the same question — *who is responsible?* — and a protocol with no operator has
no default answer. So TRACT makes the answer explicit and signed.

Each order names:

| Field | Meaning |
|---|---|
| **seller of record** | who contracts with the buyer |
| **facilitator** | the gateway, **if** it settled the payment — the marketplace-facilitator hook |
| **importer of record** | for cross-border movement |
| **responsible person** | required in-region for product safety in some jurisdictions |
| **escrow / rail** | who held funds, under which class |

A self-hosted seller taking direct payment is an ordinary distance seller. A gateway that settles
is the facilitator, and **knows** it, because the order says so. Compliance stops being a hole and
becomes a field.

## Four anchors, not two

The most common commerce-tax error is conflating the parties' locations with where a supply
happens. TRACT keeps four separate anchors, derived from different places:

| Anchor | Derived from | Governs |
|---|---|---|
| seller establishment | seller identity | licensing, seller-side registration |
| buyer residence | buyer disclosure at order | consumer-protection rights (generally non-waivable) |
| **place of supply** | **the Fulfilment axis** | VAT/GST, especially services and events |
| delivery destination | the shipping leg | customs, duty, product-safety regimes |

The worked example that forces this: **an event held in Europe, sold by a seller elsewhere.**
Admission to an event is generally taxed where the event physically takes place. Knowing both
parties' countries tells you nothing useful; only the Fulfilment object knows the venue.

By fulfilment mode: `ship` → destination; `perform-at-place` → the venue; `perform-remote` and
`digital grant` → buyer residence; rentals → collection point.

**The anchor is read out of the fulfilment object, never passed in beside it.** That sounds like an
API detail and is actually the whole guarantee: a resolver that accepts a place as an argument next
to a fulfilment that already carries one will return whatever it is handed, so a German event
resolves to South Africa on request — confidently, plausibly, wrongly. That is the exact error the
four anchors exist to make unrepresentable, and it was representable until the argument was
removed. Where a party has a genuine choice — which country a shipped order goes to — the choice is
checked against the territories the offer actually serves rather than trusted.

## What the regimes demand structurally

| Jurisdiction | Bites on | Structural implication |
|---|---|---|
| **South Africa** | electronic-transaction disclosure and cooling-off, consumer protection, POPIA, VAT, KYC on the payment side | mandatory seller-disclosure block in every offer; gateway carries KYC |
| **EU** | GDPR, platform trader-traceability, consumer rights, in-region responsible person for product safety, VAT one-stop schemes, platform reporting obligations | traceability fields non-optional; settling gateways inherit reporting duties |
| **Other African markets** | national data-protection acts, local VAT registration, regional trade frameworks | per-country geo-availability on offers |
| **New Zealand** | privacy, fair trading, consumer guarantees, GST on low-value imports | GST computed at checkout for imports |
| **Americas** | US economic-nexus and marketplace-facilitator rules, seller-traceability legislation, Canadian and Brazilian privacy law | the facilitator field is what makes state-level tax answerable at all |

> These are design inputs, not legal advice. An operator running a gateway in any of these
> jurisdictions needs its own counsel. What the protocol guarantees is that the *facts* a regulator
> asks for are present, signed, and attributable — not that any given deployment is compliant.

## Geo-availability is part of the offer

An offer declares where it may sell, what tax treatment attaches, and who the responsible person is
per region. A seller with no in-region responsible person where one is required cannot construct a
valid offer for that region — the constraint is expressed rather than deferred.

## Data protection and irrevocability

The hard conflict: erasure rights versus content-addressed, irrevocable published objects.

TRACT resolves it by construction — **no personal data enters the public quadrant.** Orders,
addresses and contact details are sealed messages between the parties, held at the edges, and
deletable there. Only products, offers, rate cards and (bounded, pseudonymous) reviews are
published. See [Trust & reviews](./trust.md) for the one residual case and how it is bounded.
