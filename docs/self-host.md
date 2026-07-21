# Self-hosting

> **Pre-alpha.** There is nothing to run yet. This page describes the intended shape so the
> deployment story is designed alongside the protocol rather than retrofitted.

## What a seller needs

At minimum: a keypair and a machine that is usually on. That is the whole requirement — no domain,
no static IP, no port forwarding, no account anywhere.

| You have | What works |
|---|---|
| a laptop that sleeps | publish a catalogue; orders arrive when you wake, via content-free push and the sender's retry queue |
| a small always-on box (Pi, NAS, VPS) | full node: catalogue, sealed orders, inventory across replicas |
| a box behind CGNAT | reachable by key via the substrate's relay ladder — no tunnel service required |
| your own domain | optional convenience; identity is the keypair, not the name |

## Reachability, concretely

The substrate climbs a ladder: direct connection, then hole-punching, then a **content-blind
circuit relay**, with a short-TTL content-blind mailbox for offline holding and content-free push
to wake a sleeping device. Durability lives in the *sender's* retry queue, not in the middle.

No ngrok, no Cloudflare Tunnel, no dynamic-DNS. Those terminate TLS and can read your orders; the
relay role is the same idea with the trust removed.

## Running a role

Every role beyond gateway needs nothing scarce:

| Role | Requirement |
|---|---|
| seller / buyer | a keypair |
| courier | a keypair and a published rate card |
| distributor | a keypair, space, and published capacity |
| index | disk and bandwidth; authoritative for nothing |
| relay / mailbox / cache | a public address |
| **gateway** | **a domain, TLS, uptime — and for escrow, licensing and a payment relationship** |

## Storefront: with or without a gateway

- **Native clients** verify everything themselves and need no gateway.
- **Browsers** cannot verify signatures, so a storefront gateway renders for them. You may run your
  own — it is ordinary HTTPS serving self-verifying objects — or use someone else's, or both.

Running your own gateway for your own store removes the trust downgrade entirely, at the cost of
operating a domain and TLS. That is the recommended path for anyone already comfortable doing so.

## Gateway isolation

A gateway terminates untrusted connections and renders untrusted merchant bundles. It **must** run
as a separate process with no access to identity keys, and it **must** give every store its own
origin. See [Gateways](./gateway.md).
