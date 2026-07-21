# Diagrams

> **There are no product screenshots here, and that is deliberate.** Soko is pre-alpha: there is no
> UI to photograph, and a mocked-up dashboard would misrepresent how far along this is. These are
> the real artefacts — the same mermaid sources the rest of the docs render, exported by
> [`tools/diagrams.mjs`](https://github.com/vul-os/soko/blob/main/tools/diagrams.mjs) so they can be
> regenerated rather than hand-maintained. Product screenshots land when there is a product.

## One cart across sovereign sellers

![One cart across independent sellers](./diagrams/cart-flow.png)

Each seller publishes a signed feed. Any node may build an index over those feeds; none is
authoritative, and a disagreement between an index and a feed resolves in favour of the feed. The
buyer's node assembles one cart across all of them and sends a **separate sealed order to each
seller**, containing only that seller's lines — so the cross-seller view exists nowhere but on the
buyer's own device.

## Where Soko sits

![Soko sits on TRACT, which sits on the DMTAP substrate](./diagrams/substrate.png)

Soko implements no cryptography. Identity, feeds, blobs, sync and reachability come from the DMTAP
substrate; TRACT adds only the commerce spine; Soko is one implementation of TRACT. A hash
construction invented in `soko-core` would be a bug, not a feature.

## The four axes

![The four axes of an offer](./diagrams/axes.png)

Goods, services, rentals, bookings and subscriptions are the same object with different axis
values. Note the dotted edge: **fulfilment also derives place of supply**, which is the tax anchor.
That is why an event held abroad is taxed at the venue regardless of where either party lives, and
why a two-anchor model cannot express it. See [Jurisdiction](./jurisdiction.md).

## Checkout across independent sellers

![Checkout sequence](./diagrams/checkout.png)

The buyer's node is the orchestrator; there is no central checkout service. Note what is
conditional: the gateway appears **only** if both parties chose escrow. Two TRACT-native parties
never need one.

Note also what is absent: any step that makes the multi-seller order atomic. One seller declining
does not roll back another's acceptance — that would need a coordinator with authority over
sovereign parties. Checkout is compensating actions, and an interface must show per-seller status
rather than a single "order placed". See [Cart & checkout](./cart.md).

## Delivery routing

![Direct versus hub consolidation](./diagrams/routing.png)

Rate cards are published as signed public objects, so the comparison runs **on the buyer's node**
over locally cached data. No quote API, no rate limit, and no third party learns what is being
bought or from whom.

The candidate set is deliberately small — a handful of hubs, not a fleet-wide optimisation — so it
can be evaluated exhaustively without an optimiser or a service. The unreliable input is
`wait_days`: when the slowest seller's parcel actually arrives. It is an estimate and must be shown
as one. See [Delivery & routing](./delivery.md).

## Regenerating

```sh
node tools/diagrams.mjs     # needs Chrome; CHROME_PATH overrides
```

Every diagram source lives in that one file, so a diagram cannot drift from a hand-edited copy of
itself.
