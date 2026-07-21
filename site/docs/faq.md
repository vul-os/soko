# FAQ

### Is this a marketplace?

No, and the word is deliberately avoided in the specification for anything TRACT defines. There is
no operator that lists sellers, ranks them, takes a cut, or can remove them. What looks like a
marketplace — a searchable view across many sellers — is an **index**: derived data any node can
rebuild, authoritative for nothing.

### Is there a token?

No. TRACT specifies no token, no coin, no ledger and no protocol fee. Settlement is a seam; the
parties choose a rail, which may be ordinary card payments, a bank transfer, or a stablecoin.

### How do I get paid?

Through whatever payment provider you already use or choose. TRACT carries signed payment
*attestations*, never funds. See [Settlement](./settlement.md).

### Do I need a domain or a static IP?

No. A keypair and a machine that is usually on is the whole requirement. Reachability comes from
the substrate's relay ladder. A domain is an optional convenience.

### Do I need ngrok or a tunnel for webhooks?

No. An order is a sealed message pushed to your key; the sender's node retries until you
acknowledge, and a content-free push can wake a sleeping device. Tunnel services terminate TLS,
which means they can read your orders — the relay role is the same idea with the trust removed.

### What stops someone listing fake products or spamming the catalogue?

A seller can only flood **their own** feed, which only their own followers and holders pay for and
can stop serving at will. There is no shared feed to spam and no fan-out amplification. Indexes
apply their own admission policy, and different indexes will differ — which is the point.

### Can someone claim a GTIN that isn't theirs?

Yes, and TRACT says so. External identifiers are **claims**, never identity. The defence is the
top rung of the ladder: a manufacturer signs the canonical record for its own product, so a
reseller can add an offer but cannot misdescribe the thing. See [Catalogue](./catalogue.md).

### Can a seller be delisted?

Not from the network — nobody holds that permission. An individual index or gateway can stop
serving you, and your store keeps existing; another index or gateway will differ. That is a
meaningful difference from a platform suspension, which ends your business.

### How is this different from Shopify?

Shopify owns your catalogue, your customer list, your storefront and your ability to trade. Soko
owns none of them. The concrete difference is what leaving costs: an export and a rebuild, versus a
DNS change. See the table in [Overview](./overview.md).

### What can Shopify do that this cannot?

Give you one authoritative star rating, person-level analytics and retargeting, guaranteed uptime
someone else operates, and a support number. Those are real, and pretending otherwise would be
dishonest. See [Threat model](./threat-model.md).

### What can this do that Shopify structurally cannot?

One cart across independent sellers, bundles whose components come from different sellers, a
product record the *manufacturer* signs, price comparison across every seller of a SKU with nobody
taking a cut, and a cart and purchase history that survive any store closing.

### Does it work for services, not just products?

Yes — that is the point of the four axes. Bookings, rentals, subscriptions, memberships, metered
usage and B2B quote-based pricing are the same object with different axis values, not plugins. See
[Offers](./offers.md).

### Which countries does it work in?

By design, all of them. Jurisdiction is a first-class field rather than a platform's terms of
service, with four separate anchors for seller, buyer, place of supply and delivery destination.
Escrow availability is narrower, because escrow operators are licensed per jurisdiction. See
[Jurisdiction](./jurisdiction.md).

### Hasn't this been tried and failed?

Yes. **OpenBazaar** was the closest relative — signed objects, content-addressed listings, keypair
identity, no operator — and it shut down in 2021 having moved about US$86,000 over 14 months, with
~80 users online at a time and one vendor faking 60% of measured sales value. Its catalogues
disappeared when merchant nodes went offline, and its default search engine became the gatekeeper
the design was meant to avoid.

Meanwhile **Beckn/ONDC**, the largest live decentralized-commerce network, works precisely because
it reintroduced a central registry with approval-gated enrollment.

That evidence is recorded in the specification rather than omitted from it, and it is the reason
several claims on these pages are hedged where a marketing page would not hedge them. What is
different here — purchase-attested reviews, gateways as a liveness backstop, buyer-held carts,
sealed orders with signed transitions — are answers to specific OpenBazaar failures. Whether they
are *sufficient* answers is not yet demonstrated by anything.

### Can I use it today?

No. It is pre-alpha and the specification is being written first, on purpose.
