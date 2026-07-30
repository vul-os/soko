# Security

Soko is the reference **implementation** of
[TRACT](https://github.com/vul-os/kotva/tree/main/profiles/tract). This page covers vulnerabilities
in the code in this repository. A defect in the protocol itself — an under-specified check, a
mechanism that fails open, a trust assumption TRACT never states — belongs with the specification
instead, even if you found it by reading this code: report it under
[TRACT's security policy](https://github.com/vul-os/kotva/blob/main/profiles/tract/SECURITY.md) in
the `vul-os/kotva` repository, which is where the TRACT profile lives.

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

- Preferred: [GitHub private vulnerability reporting](https://github.com/vul-os/soko/security/advisories/new)
  on `vul-os/soko`.
- Alternatively, email **vulosorg@gmail.com** with `[soko security]` in the subject.

Include what you can: the affected crate, reproduction steps, and the impact as you understand it.
Please give a reasonable window before public disclosure.

There is no release yet, so there is no supported-version table worth pretending exists. Every
report is against `main`, and every fix lands there.

## What counts as a security bug here

TRACT's honest-limits discipline carries over: a limit that was known and left undocumented is
treated as seriously as a wrong byte. For Soko specifically, report:

- **Personal data reachable from the `public` module.** `soko-core` splits `public`
  (content-addressed, irrevocable) from `sealed` (encrypted, deletable at the edges) precisely so
  this cannot happen. A `public` type that can carry a name, address, or contact detail — or a code
  path that writes sealed data into a published object — is a critical bug, not a design trade-off.
  See [docs/architecture.md](docs/architecture.md#the-publicsealed-split-is-structural).
- **A silent fail-open.** Anywhere a check that should refuse instead defaults to permitting: an
  oversell past bounded-counter inventory, a currency mismatch that converts instead of refusing, a
  place-of-supply guess where the protocol requires none.
- **A downgrade path applied without both parties choosing it.** In particular: rail-class
  substitution (a `NonCustodialFinal` request silently served over a `CustodialReversible` rail or
  vice versa) and escrow-scope mismatch (a checkout proceeding through an operator whose declared
  scope does not cover the trade). See [docs/settlement.md](docs/settlement.md).
- **An origin-isolation violation in the gateway.** Merchant render bundles are untrusted code.
  Two stores sharing an origin means one bundle can read another store's cart and session — see
  [docs/gateway.md](docs/gateway.md#origin-isolation-is-mandatory).
- **A gateway process gaining access to identity keys or the object store.** The gateway is
  specified to run as a separate process with neither (§12.4). A code path that hands it either is
  a critical bug regardless of whether it is currently exploitable.
- **A dependency added to `soko-seam`.** Not exploitable in the usual sense, but a supply-chain
  problem: the seam exists so that implementing settlement or storefront rendering costs nothing,
  and a dependency there is inherited by every implementor of the seam. CI checks `cargo tree -p
  soko-seam` is one line; report it anyway if you find a path around the check.

## Known structural exposures

These are consequences of the design TRACT specifies, not implementation bugs, and Soko does not
attempt to fix them — they are documented instead. Full detail in
[docs/threat-model.md](docs/threat-model.md):

- **The gateway is trusted by browsers.** A shopper with no keypair cannot verify a signature.
  Mitigated by universal re-renderability, not removed — unlike DMTAP's mail gateway, this one does
  not self-extinguish as adoption grows, because browsers are permanent.
- **Escrow operators hold funds.** Never identity keys, chosen per order, every ruling published —
  but they are custodians, and physical custody more broadly cannot be made trustless.
- **Published objects are irrevocable.** This is the entire reason the `public`/`sealed` split
  exists in the first place, and why reviews are the one bounded exception, handled with
  pseudonymous subkeys and superseding tombstones rather than deletion.
- **Reputation is manipulable at a cost.** Purchase attestation raises the price of ballot-stuffing
  and leaves a trail; it does not establish counterparty independence.

If your report is about one of these categories in the abstract, it is already known — a useful
report shows a concrete case Soko fails to bound the way the design intends (for example, a
re-render comparison that cannot actually detect a dishonest gateway).

## Cryptography

Soko implements none. Identity, signing, content addressing, feeds and sync come from the
[DMTAP substrate](https://github.com/vul-os/kotva). A hash construction or signature framing found
inside `soko-core` is itself the bug — report it here, since it would be an implementation error
even though the primitive it should have used is specified elsewhere.
